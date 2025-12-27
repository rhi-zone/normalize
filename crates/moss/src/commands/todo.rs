//! Todo command - structured TODO.md editing without content loss
//!
//! Detects common TODO.md formats automatically:
//! - Section headers: `##`, `#`, `###` with common names
//! - Item formats: checkboxes `- [ ]`, numbers `1.`, bullets `-`
//! - Preserves user's existing format when adding items

use std::fs;
use std::path::Path;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum TodoAction {
    /// List items in the primary section (default)
    List {
        /// Show full TODO.md content
        #[arg(short, long)]
        full: bool,
    },
    /// Add an item to the primary section
    Add {
        /// Item text to add
        text: String,
    },
    /// Mark an item as done (fuzzy text match)
    Done {
        /// Text to match (case-insensitive substring)
        query: String,
    },
    /// Remove an item (fuzzy text match)
    Rm {
        /// Text to match (case-insensitive substring)
        query: String,
    },
}

/// Detected item format in a section
#[derive(Debug, Clone, Copy, PartialEq)]
enum ItemFormat {
    Checkbox, // - [ ] item / - [x] item
    Numbered, // 1. item
    Bullet,   // - item
    Asterisk, // * item
    Plain,    // just text lines
}

/// A detected section in the TODO file
#[derive(Debug)]
struct Section {
    name: String,
    header_line: usize,
    header_level: usize, // number of # chars
    items: Vec<Item>,
    format: ItemFormat,
}

/// An item within a section
#[derive(Debug)]
struct Item {
    line_num: usize,
    text: String,
    done: bool,
    raw_line: String,
}

/// Priority names for the "primary" section (checked in order)
const PRIMARY_SECTION_NAMES: &[&str] = &[
    "next up",
    "next",
    "todo",
    "tasks",
    "in progress",
    "current",
    "active",
];

/// Parse the entire TODO file structure
fn parse_todo(content: &str) -> Vec<Section> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut current_section: Option<Section> = None;

    for (line_num, line) in lines.iter().enumerate() {
        // Detect section headers
        if let Some((level, name)) = parse_header(line) {
            // Save previous section
            if let Some(mut section) = current_section.take() {
                section.format = detect_format(&section.items);
                sections.push(section);
            }
            current_section = Some(Section {
                name,
                header_line: line_num,
                header_level: level,
                items: Vec::new(),
                format: ItemFormat::Plain,
            });
            continue;
        }

        // Parse items within current section
        if let Some(ref mut section) = current_section {
            if let Some(item) = parse_item(line, line_num) {
                section.items.push(item);
            }
        }
    }

    // Don't forget the last section
    if let Some(mut section) = current_section {
        section.format = detect_format(&section.items);
        sections.push(section);
    }

    sections
}

/// Parse a markdown header, returns (level, name)
fn parse_header(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }

    let level = trimmed.chars().take_while(|&c| c == '#').count();
    let name = trimmed[level..].trim().to_string();

    if name.is_empty() {
        return None;
    }

    Some((level, name))
}

/// Parse a line as an item
fn parse_item(line: &str, line_num: usize) -> Option<Item> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Checkbox: - [ ] or - [x]
    if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        return Some(Item {
            line_num,
            text: rest.to_string(),
            done: true,
            raw_line: line.to_string(),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return Some(Item {
            line_num,
            text: rest.to_string(),
            done: false,
            raw_line: line.to_string(),
        });
    }

    // Bullet: - item
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(Item {
            line_num,
            text: rest.to_string(),
            done: false,
            raw_line: line.to_string(),
        });
    }

    // Asterisk: * item
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return Some(Item {
            line_num,
            text: rest.to_string(),
            done: false,
            raw_line: line.to_string(),
        });
    }

    // Numbered: 1. item (handles multi-digit)
    if let Some((num_part, rest)) = trimmed.split_once(". ") {
        if num_part.chars().all(|c| c.is_ascii_digit()) && !num_part.is_empty() {
            return Some(Item {
                line_num,
                text: rest.to_string(),
                done: false,
                raw_line: line.to_string(),
            });
        }
    }

    None
}

/// Detect the predominant format in a list of items
fn detect_format(items: &[Item]) -> ItemFormat {
    if items.is_empty() {
        return ItemFormat::Bullet; // sensible default
    }

    let mut checkbox_count = 0;
    let mut numbered_count = 0;
    let mut bullet_count = 0;
    let mut asterisk_count = 0;

    for item in items {
        let trimmed = item.raw_line.trim();
        if trimmed.starts_with("- [") {
            checkbox_count += 1;
        } else if trimmed.starts_with("- ") {
            bullet_count += 1;
        } else if trimmed.starts_with("* ") {
            asterisk_count += 1;
        } else if trimmed
            .split_once('.')
            .map(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
        {
            numbered_count += 1;
        }
    }

    // Return the most common format
    let max = checkbox_count
        .max(numbered_count)
        .max(bullet_count)
        .max(asterisk_count);

    if max == 0 {
        ItemFormat::Bullet
    } else if checkbox_count == max {
        ItemFormat::Checkbox
    } else if numbered_count == max {
        ItemFormat::Numbered
    } else if asterisk_count == max {
        ItemFormat::Asterisk
    } else {
        ItemFormat::Bullet
    }
}

/// Find the primary section (the one to use for add/done operations)
fn find_primary_section(sections: &[Section]) -> Option<usize> {
    // First, look for priority names
    for priority_name in PRIMARY_SECTION_NAMES {
        for (i, section) in sections.iter().enumerate() {
            if section.name.to_lowercase().contains(priority_name) {
                return Some(i);
            }
        }
    }

    // Fall back to first section with items, or just first section
    sections
        .iter()
        .position(|s| !s.items.is_empty())
        .or_else(|| if sections.is_empty() { None } else { Some(0) })
}

/// Format a new item in the given format
fn format_item(text: &str, format: ItemFormat, number: Option<usize>) -> String {
    match format {
        ItemFormat::Checkbox => format!("- [ ] {}", text),
        ItemFormat::Numbered => format!("{}. {}", number.unwrap_or(1), text),
        ItemFormat::Bullet => format!("- {}", text),
        ItemFormat::Asterisk => format!("* {}", text),
        ItemFormat::Plain => text.to_string(),
    }
}

/// Add an item to a section
fn add_item(content: &str, section_name: Option<&str>, item_text: &str) -> Result<String, String> {
    let sections = parse_todo(content);

    let section_idx = if let Some(name) = section_name {
        sections
            .iter()
            .position(|s| s.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| format!("Section '{}' not found", name))?
    } else {
        find_primary_section(&sections).ok_or("No sections found in TODO.md")?
    };

    let section = &sections[section_idx];
    let format = section.format;

    // Find insertion point (after last item in section, or after header)
    let insert_after = section
        .items
        .last()
        .map(|i| i.line_num)
        .unwrap_or(section.header_line);

    // Calculate next number if numbered
    let next_num = if format == ItemFormat::Numbered {
        Some(section.items.len() + 1)
    } else {
        None
    };

    let new_line = format_item(item_text, format, next_num);

    // Build new content
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();

    for (i, line) in lines.iter().enumerate() {
        result.push_str(line);
        result.push('\n');
        if i == insert_after {
            result.push_str(&new_line);
            result.push('\n');
        }
    }

    // Handle edge case: inserting at end of file
    if insert_after >= lines.len() {
        result.push_str(&new_line);
        result.push('\n');
    }

    Ok(result)
}

/// Find item by fuzzy text match
fn find_item_by_text<'a>(section: &'a Section, query: &str) -> Result<&'a Item, String> {
    let query_lower = query.to_lowercase();

    // Exact substring match first
    let matches: Vec<_> = section
        .items
        .iter()
        .filter(|i| i.text.to_lowercase().contains(&query_lower))
        .collect();

    match matches.len() {
        0 => Err(format!("No item matching '{}' found", query)),
        1 => Ok(matches[0]),
        _ => {
            // Multiple matches - show them
            let mut msg = format!("Multiple items match '{}'. Be more specific:\n", query);
            for (i, item) in matches.iter().enumerate() {
                msg.push_str(&format!("  {}. {}\n", i + 1, item.text));
            }
            Err(msg)
        }
    }
}

/// Mark an item as done (toggle checkbox or add [x])
fn mark_item_done(content: &str, query: &str) -> Result<(String, String), String> {
    let sections = parse_todo(content);
    let section_idx = find_primary_section(&sections).ok_or("No sections found")?;
    let section = &sections[section_idx];

    let item = find_item_by_text(section, query)?;
    let lines: Vec<&str> = content.lines().collect();

    // Build new content with item marked as done
    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i == item.line_num {
            // Transform the line based on format
            let new_line = mark_line_done(line);
            result.push_str(&new_line);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    Ok((result, item.text.clone()))
}

/// Transform a line to mark it as done
fn mark_line_done(line: &str) -> String {
    let trimmed = line.trim();

    // Already done
    if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
        return line.to_string();
    }

    // Checkbox: - [ ] -> - [x]
    if trimmed.starts_with("- [ ] ") {
        return line.replace("- [ ] ", "- [x] ");
    }

    // Other formats: prepend [x]
    // For bullets: - item -> - [x] item
    if let Some(rest) = trimmed.strip_prefix("- ") {
        let indent = &line[..line.len() - line.trim_start().len()];
        return format!("{}- [x] {}", indent, rest);
    }

    // For numbered: 1. item -> 1. [x] item
    if let Some((num, rest)) = trimmed.split_once(". ") {
        if num.chars().all(|c| c.is_ascii_digit()) {
            let indent = &line[..line.len() - line.trim_start().len()];
            return format!("{}{}. [x] {}", indent, num, rest);
        }
    }

    // Fallback: just return as-is
    line.to_string()
}

/// Remove an item by text match
fn remove_item(content: &str, query: &str) -> Result<(String, String), String> {
    let sections = parse_todo(content);
    let section_idx = find_primary_section(&sections).ok_or("No sections found")?;
    let section = &sections[section_idx];

    let item = find_item_by_text(section, query)?;
    let lines: Vec<&str> = content.lines().collect();

    // Build new content without the item line
    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i != item.line_num {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    // Renumber if using numbered format
    if section.format == ItemFormat::Numbered {
        result = renumber_section(&result, &section.name);
    }

    Ok((result, item.text.clone()))
}

/// Renumber items in a section
fn renumber_section(content: &str, section_name: &str) -> String {
    let mut in_section = false;
    let mut item_num = 1;
    let mut result = String::new();

    for line in content.lines() {
        if let Some((_, name)) = parse_header(line) {
            in_section = name.to_lowercase().contains(&section_name.to_lowercase());
            item_num = 1;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_section {
            let trimmed = line.trim();
            // Check if it's a numbered item
            if let Some((num_str, rest)) = trimmed.split_once(". ") {
                if num_str.chars().all(|c| c.is_ascii_digit()) {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    result.push_str(&format!("{}{}. {}\n", indent, item_num, rest));
                    item_num += 1;
                    continue;
                }
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // Remove trailing newline if content didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Main command handler
pub fn cmd_todo(action: Option<TodoAction>, json: bool, root: &Path) -> i32 {
    let todo_path = root.join("TODO.md");

    if !todo_path.exists() {
        eprintln!("No TODO.md found in {}", root.display());
        return 1;
    }

    let content = match fs::read_to_string(&todo_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading TODO.md: {}", e);
            return 1;
        }
    };

    match action {
        Some(TodoAction::Add { text }) => match add_item(&content, None, &text) {
            Ok(new_content) => {
                if let Err(e) = fs::write(&todo_path, &new_content) {
                    eprintln!("Error writing TODO.md: {}", e);
                    return 1;
                }
                if json {
                    println!("{}", serde_json::json!({"status": "added", "item": text}));
                } else {
                    let sections = parse_todo(&content);
                    let section_name = find_primary_section(&sections)
                        .map(|i| sections[i].name.as_str())
                        .unwrap_or("TODO");
                    println!("Added to {}: {}", section_name, text);
                }
                0
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },

        Some(TodoAction::Done { query }) => match mark_item_done(&content, &query) {
            Ok((new_content, completed_item)) => {
                if let Err(e) = fs::write(&todo_path, &new_content) {
                    eprintln!("Error writing TODO.md: {}", e);
                    return 1;
                }
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"status": "completed", "item": completed_item})
                    );
                } else {
                    println!("Marked done: {}", completed_item);
                }
                0
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },

        Some(TodoAction::Rm { query }) => match remove_item(&content, &query) {
            Ok((new_content, removed_item)) => {
                if let Err(e) = fs::write(&todo_path, &new_content) {
                    eprintln!("Error writing TODO.md: {}", e);
                    return 1;
                }
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"status": "removed", "item": removed_item})
                    );
                } else {
                    println!("Removed: {}", removed_item);
                }
                0
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },

        None | Some(TodoAction::List { full: false }) => {
            let sections = parse_todo(&content);

            if json {
                let sections_json: Vec<_> = sections
                    .iter()
                    .map(|s| {
                        let items: Vec<_> = s
                            .items
                            .iter()
                            .enumerate()
                            .map(|(i, item)| {
                                serde_json::json!({
                                    "index": i + 1,
                                    "text": item.text,
                                    "done": item.done
                                })
                            })
                            .collect();
                        serde_json::json!({
                            "name": s.name,
                            "format": format!("{:?}", s.format),
                            "items": items
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&sections_json).unwrap());
            } else {
                // Show primary section by default
                if let Some(idx) = find_primary_section(&sections) {
                    let section = &sections[idx];
                    println!("{} {}\n", "#".repeat(section.header_level), section.name);
                    if section.items.is_empty() {
                        println!("  (no items)");
                    } else {
                        for (i, item) in section.items.iter().enumerate() {
                            let marker = if item.done { "[x]" } else { "   " };
                            println!("{}  {}. {}", marker, i + 1, item.text);
                        }
                    }
                    // Show section count
                    if sections.len() > 1 {
                        eprintln!(
                            "\n({} sections total, use --full to see all)",
                            sections.len()
                        );
                    }
                } else {
                    println!("No sections found in TODO.md");
                }
            }
            0
        }

        Some(TodoAction::List { full: true }) => {
            if json {
                println!("{}", serde_json::json!({"content": content}));
            } else {
                print!("{}", content);
            }
            0
        }
    }
}
