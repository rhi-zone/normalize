//! Duplicate function and type detection.

use crate::extract::Extractor;
use crate::filter::Filter;
use crate::output::OutputFormatter;
use crate::parsers;
use normalize_languages::support_for_path;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// A group of duplicate functions
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicateFunctionGroup {
    #[serde(serialize_with = "serialize_hash")]
    hash: u64,
    locations: Vec<DuplicateFunctionLocation>,
    line_count: usize,
}

fn serialize_hash<S>(hash: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{:016x}", hash))
}

/// Location of a duplicate function instance
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicateFunctionLocation {
    file: String,
    symbol: String,
    start_line: usize,
    end_line: usize,
}

/// Duplicate functions analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateFunctionsReport {
    files_scanned: usize,
    functions_hashed: usize,
    #[serde(skip)]
    total_duplicates: usize,
    duplicated_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    groups: Vec<DuplicateFunctionGroup>,
    #[serde(skip)]
    root: PathBuf,
    #[serde(skip)]
    show_source: bool,
}

impl OutputFormatter for DuplicateFunctionsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Duplicate Function Detection".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned: {}", self.files_scanned));
        lines.push(format!("Functions hashed: {}", self.functions_hashed));
        lines.push(format!("Duplicate groups: {}", self.groups.len()));
        lines.push(format!("Total duplicates: {}", self.total_duplicates));
        lines.push(format!("Duplicated lines: ~{}", self.duplicated_lines));
        lines.push(String::new());

        if self.groups.is_empty() {
            lines.push("No duplicate functions detected.".to_string());
        } else {
            lines.push("Duplicate Groups (sorted by size):".to_string());
            lines.push(String::new());

            for (i, group) in self.groups.iter().take(20).enumerate() {
                lines.push(format!(
                    "{}. {} lines, {} instances:",
                    i + 1,
                    group.line_count,
                    group.locations.len()
                ));

                for loc in &group.locations {
                    lines.push(format!(
                        "   {}:{}-{} ({})",
                        loc.file, loc.start_line, loc.end_line, loc.symbol
                    ));

                    if self.show_source {
                        let file_path = self.root.join(&loc.file);
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            let file_lines: Vec<&str> = content.lines().collect();
                            let start = loc.start_line.saturating_sub(1);
                            let end = loc.end_line.min(file_lines.len());
                            for (j, line) in file_lines[start..end].iter().enumerate() {
                                lines.push(format!("        {:4} │ {}", start + j + 1, line));
                            }
                            lines.push(String::new());
                        }
                    }
                }
                lines.push(String::new());
            }

            if self.groups.len() > 20 {
                lines.push(format!("... and {} more groups", self.groups.len() - 20));
            }
        }

        lines.join("\n")
    }
}

/// Type information
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
struct TypeInfo {
    file: String,
    name: String,
    start_line: usize,
    fields: Vec<String>,
}

/// A pair of duplicate types
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicatePair {
    type1: TypeInfo,
    type2: TypeInfo,
    overlap_percent: usize,
    common_fields: Vec<String>,
}

/// Duplicate types analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateTypesReport {
    files_scanned: usize,
    types_analyzed: usize,
    min_overlap_percent: usize,
    duplicates: Vec<DuplicatePair>,
}

impl OutputFormatter for DuplicateTypesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Duplicate Type Detection".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned: {}", self.files_scanned));
        lines.push(format!("Types analyzed: {}", self.types_analyzed));
        lines.push(format!("Duplicate pairs: {}", self.duplicates.len()));
        lines.push(format!("Min overlap: {}%", self.min_overlap_percent));
        lines.push(String::new());

        if self.duplicates.is_empty() {
            lines.push("No duplicate types detected.".to_string());
        } else {
            lines.push("Potential Duplicates (sorted by overlap):".to_string());
            lines.push(String::new());

            for (i, dup) in self.duplicates.iter().take(20).enumerate() {
                lines.push(format!(
                    "{}. {}% overlap ({} common fields):",
                    i + 1,
                    dup.overlap_percent,
                    dup.common_fields.len()
                ));
                lines.push(format!(
                    "   {} ({}:{}) - {} fields",
                    dup.type1.name,
                    dup.type1.file,
                    dup.type1.start_line,
                    dup.type1.fields.len()
                ));
                lines.push(format!(
                    "   {} ({}:{}) - {} fields",
                    dup.type2.name,
                    dup.type2.file,
                    dup.type2.start_line,
                    dup.type2.fields.len()
                ));
                lines.push(format!("   Common: {}", dup.common_fields.join(", ")));
                lines.push(String::new());
            }

            if self.duplicates.len() > 20 {
                lines.push(format!("... and {} more pairs", self.duplicates.len() - 20));
            }
        }

        lines.join("\n")
    }
}

/// Result from duplicate function detection.
pub struct DuplicateFunctionResult {
    /// Exit code (0 = success, non-zero = violations found)
    pub exit_code: i32,
    /// Number of duplicate groups found
    pub group_count: usize,
}

/// Load allowed duplicate function locations from .normalize/duplicate-functions-allow file
fn load_duplicate_functions_allowlist(root: &Path) -> HashSet<String> {
    let path = root.join(".normalize/duplicate-functions-allow");
    let mut allowed = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            allowed.insert(line.to_string());
        }
    }
    allowed
}

/// Detect all duplicate function groups in the codebase (before filtering by allowlist)
fn detect_duplicate_function_groups(
    root: &Path,
    elide_identifiers: bool,
    elide_literals: bool,
    min_lines: usize,
) -> Vec<DuplicateFunctionGroup> {
    let extractor = Extractor::new();

    let mut hash_groups: HashMap<u64, Vec<DuplicateFunctionLocation>> = HashMap::new();

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        let tree = match parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        let result = extractor.extract(path, &content);

        for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
            let kind = sym.kind.as_str();
            if kind != "function" && kind != "method" {
                continue;
            }

            if let Some(node) = find_function_node(&tree, sym.start_line) {
                let line_count = sym.end_line.saturating_sub(sym.start_line) + 1;
                if line_count < min_lines {
                    continue;
                }

                let hash = compute_function_hash(
                    &node,
                    content.as_bytes(),
                    elide_identifiers,
                    elide_literals,
                );

                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string();

                hash_groups
                    .entry(hash)
                    .or_default()
                    .push(DuplicateFunctionLocation {
                        file: rel_path,
                        symbol: sym.name.clone(),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                    });
            }
        }
    }

    // Filter to groups with 2+ instances (actual duplicates)
    let mut groups: Vec<DuplicateFunctionGroup> = hash_groups
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .map(|(hash, locations)| {
            let line_count = locations
                .first()
                .map(|l| l.end_line - l.start_line + 1)
                .unwrap_or(0);
            DuplicateFunctionGroup {
                hash,
                locations,
                line_count,
            }
        })
        .collect();

    // Sort by line count (larger duplicates first), then by number of instances
    groups.sort_by(|a, b| {
        b.line_count
            .cmp(&a.line_count)
            .then_with(|| b.locations.len().cmp(&a.locations.len()))
    });

    groups
}

/// Allow a specific duplicate function group by adding it to .normalize/duplicate-functions-allow
pub fn cmd_allow_duplicate_function(
    root: &Path,
    location: &str,
    reason: Option<&str>,
    elide_identifiers: bool,
    elide_literals: bool,
    min_lines: usize,
) -> i32 {
    // Detect all duplicate function groups
    let all_groups =
        detect_duplicate_function_groups(root, elide_identifiers, elide_literals, min_lines);

    // Find the group containing this location
    // Support both formats:
    //   file:symbol (e.g., src/foo.rs:my_func)
    //   file:start-end (e.g., src/foo.rs:10-20) - matches line range from output
    let target_group = all_groups.iter().find(|g| {
        g.locations.iter().any(|loc| {
            // Try file:symbol format first
            let entry = format!("{}:{}", loc.file, loc.symbol);
            if entry == location {
                return true;
            }
            // Try file:start-end format (copy-paste from output)
            let range_entry = format!("{}:{}-{}", loc.file, loc.start_line, loc.end_line);
            range_entry == location
        })
    });

    let group = match target_group {
        Some(g) => g,
        None => {
            eprintln!("No duplicate function group found containing: {}", location);
            eprintln!("Run `normalize analyze --duplicate-functions` to see available groups.");
            return 1;
        }
    };

    // Load existing allowlist to check for overlap
    let allowlist_path = root.join(".normalize/duplicate-functions-allow");
    let existing_content = std::fs::read_to_string(&allowlist_path).unwrap_or_default();
    let existing_lines: Vec<&str> = existing_content.lines().collect();

    // Check if any entries from this group already exist
    let mut has_existing = false;
    let mut insert_after: Option<usize> = None;

    for loc in &group.locations {
        let entry = format!("{}:{}", loc.file, loc.symbol);
        for (i, line) in existing_lines.iter().enumerate() {
            if line.trim() == entry {
                has_existing = true;
                insert_after = Some(insert_after.map_or(i, |prev| prev.max(i)));
            }
        }
    }

    // Require reason for new groups
    if !has_existing && reason.is_none() {
        eprintln!("Reason required for new groups. Use --reason \"...\"");
        return 1;
    }

    // Build entries to add
    let mut to_add: Vec<String> = Vec::new();
    for loc in &group.locations {
        let entry = format!("{}:{}", loc.file, loc.symbol);
        if !existing_lines.iter().any(|l| l.trim() == entry) {
            to_add.push(entry);
        }
    }

    if to_add.is_empty() {
        println!("All entries from this group are already allowed.");
        return 0;
    }

    // Build new content with smart insertion
    let mut new_lines: Vec<String> = existing_lines.iter().map(|s| s.to_string()).collect();

    if let Some(idx) = insert_after {
        // Insert near existing entries from this group
        let insert_pos = idx + 1;
        for (i, entry) in to_add.iter().enumerate() {
            new_lines.insert(insert_pos + i, entry.clone());
        }
    } else {
        // Append at end with reason as comment
        if !new_lines.is_empty() && !new_lines.last().is_none_or(|l| l.is_empty()) {
            new_lines.push(String::new());
        }
        if let Some(r) = reason {
            new_lines.push(format!("# {}", r));
        }
        for entry in &to_add {
            new_lines.push(entry.clone());
        }
    }

    // Write back
    let new_content = new_lines.join("\n") + "\n";
    if let Err(e) = std::fs::write(&allowlist_path, new_content) {
        eprintln!(
            "Failed to write .normalize/duplicate-functions-allow: {}",
            e
        );
        return 1;
    }

    println!(
        "Added {} entries to .normalize/duplicate-functions-allow:",
        to_add.len()
    );
    for entry in &to_add {
        println!("  {}", entry);
    }

    0
}

/// Detect duplicate functions.
pub fn cmd_duplicate_functions_with_count(
    root: &Path,
    elide_identifiers: bool,
    elide_literals: bool,
    show_source: bool,
    min_lines: usize,
    format: &crate::output::OutputFormat,
    filter: Option<&Filter>,
) -> DuplicateFunctionResult {
    let json = format.is_json();
    let extractor = Extractor::new();

    let allowlist = load_duplicate_functions_allowlist(root);

    // Collect function hashes: hash -> [(file, symbol, start, end)]
    let mut hash_groups: HashMap<u64, Vec<DuplicateFunctionLocation>> = HashMap::new();
    let mut files_scanned = 0;
    let mut functions_hashed = 0;

    // Walk source files, respecting .gitignore
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();

        // Apply filter if specified
        if let Some(f) = filter {
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if !f.matches(rel_path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        // Parse the file
        let tree = match parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        files_scanned += 1;

        // Extract symbols to find functions/methods
        let result = extractor.extract(path, &content);

        // Find and hash each function/method
        for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
            let kind = sym.kind.as_str();
            if kind != "function" && kind != "method" {
                continue;
            }

            // Find the function node
            if let Some(node) = find_function_node(&tree, sym.start_line) {
                let line_count = sym.end_line.saturating_sub(sym.start_line) + 1;
                if line_count < min_lines {
                    continue;
                }

                let hash = compute_function_hash(
                    &node,
                    content.as_bytes(),
                    elide_identifiers,
                    elide_literals,
                );
                functions_hashed += 1;

                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string();

                hash_groups
                    .entry(hash)
                    .or_default()
                    .push(DuplicateFunctionLocation {
                        file: rel_path,
                        symbol: sym.name.clone(),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                    });
            }
        }
    }

    // Filter to groups with 2+ instances (actual duplicates)
    // Skip groups where ALL locations are in the allowlist
    let mut groups: Vec<DuplicateFunctionGroup> = hash_groups
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .filter(|(_, locs)| {
            // Keep if any location is NOT allowed
            locs.iter()
                .any(|loc| !allowlist.contains(&format!("{}:{}", loc.file, loc.symbol)))
        })
        .map(|(hash, locations)| {
            let line_count = locations
                .first()
                .map(|l| l.end_line - l.start_line + 1)
                .unwrap_or(0);
            DuplicateFunctionGroup {
                hash,
                locations,
                line_count,
            }
        })
        .collect();

    // Sort by line count (larger duplicates first), then by number of instances
    groups.sort_by(|a, b| {
        b.line_count
            .cmp(&a.line_count)
            .then_with(|| b.locations.len().cmp(&a.locations.len()))
    });

    let total_duplicates: usize = groups.iter().map(|g| g.locations.len()).sum();
    let duplicated_lines: usize = groups
        .iter()
        .map(|g| g.line_count * g.locations.len())
        .sum();

    let group_count = groups.len();

    let report = DuplicateFunctionsReport {
        files_scanned,
        functions_hashed,
        total_duplicates,
        duplicated_lines,
        elide_identifiers,
        elide_literals,
        groups,
        root: root.to_path_buf(),
        show_source,
    };

    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    let exit_code = if group_count == 0 { 0 } else { 1 };
    DuplicateFunctionResult {
        exit_code,
        group_count,
    }
}

/// Detect duplicate type definitions (structs with similar fields)
pub fn cmd_duplicate_types(
    root: &Path,
    config_root: &Path,
    min_overlap_percent: usize,
    format: &crate::output::OutputFormat,
) -> i32 {
    let json = format.is_json();
    use regex::Regex;

    let extractor = Extractor::new();

    // Load allowlist
    let allowlist_path = config_root.join(".normalize/duplicate-types-allow");
    let allowed_pairs: HashSet<(String, String)> = std::fs::read_to_string(&allowlist_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() == 2 {
                // Store in sorted order for consistent matching
                let (a, b) = if parts[0] < parts[1] {
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    (parts[1].to_string(), parts[0].to_string())
                };
                Some((a, b))
            } else {
                None
            }
        })
        .collect();

    // Collect types with their fields
    let mut types: Vec<TypeInfo> = Vec::new();
    let mut files_scanned = 0;

    // Regex to extract field names from struct definitions
    // Matches patterns like: field_name: Type or pub field_name: Type
    let field_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(\w+)\s*:\s*\S").unwrap();

    // Collect files to scan - either a single file or walk a directory
    let files: Vec<PathBuf> = if root.is_file() {
        vec![root.to_path_buf()]
    } else {
        ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                path.is_file() && super::is_source_file(path)
            })
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    for path in &files {
        let path = path.as_path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        files_scanned += 1;

        // Extract symbols
        let result = extractor.extract(path, &content);
        let lines: Vec<&str> = content.lines().collect();

        // Find type symbols (struct, class, interface, etc.)
        for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
            let kind = sym.kind.as_str();
            if !matches!(kind, "struct" | "class" | "interface" | "type") {
                continue;
            }

            // Extract field names from source
            let start = sym.start_line.saturating_sub(1);
            let end = sym.end_line.min(lines.len());
            let source: String = lines[start..end].join("\n");

            let fields: Vec<String> = field_re
                .captures_iter(&source)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .collect();

            // Skip types with too few fields
            if fields.len() < 2 {
                continue;
            }

            let rel_path = if root.is_file() {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
            } else {
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            };

            types.push(TypeInfo {
                file: rel_path,
                name: sym.name.clone(),
                start_line: sym.start_line,
                fields,
            });
        }
    }

    // Find duplicate pairs based on field overlap
    let mut duplicates: Vec<DuplicatePair> = Vec::new();

    for i in 0..types.len() {
        for j in (i + 1)..types.len() {
            let t1 = &types[i];
            let t2 = &types[j];

            // Skip if same name (intentional reimplementation)
            if t1.name == t2.name {
                continue;
            }

            // Skip if pair is in allowlist
            let pair_key = if t1.name < t2.name {
                (t1.name.clone(), t2.name.clone())
            } else {
                (t2.name.clone(), t1.name.clone())
            };
            if allowed_pairs.contains(&pair_key) {
                continue;
            }

            // Calculate field overlap
            let set1: HashSet<_> = t1.fields.iter().collect();
            let set2: HashSet<_> = t2.fields.iter().collect();

            let common: Vec<String> = set1.intersection(&set2).map(|s| (*s).clone()).collect();

            let min_size = t1.fields.len().min(t2.fields.len());
            let overlap_percent = if min_size > 0 {
                (common.len() * 100) / min_size
            } else {
                0
            };

            if overlap_percent >= min_overlap_percent {
                duplicates.push(DuplicatePair {
                    type1: t1.clone(),
                    type2: t2.clone(),
                    overlap_percent,
                    common_fields: common,
                });
            }
        }
    }

    // Sort by overlap percentage (highest first)
    duplicates.sort_by(|a, b| b.overlap_percent.cmp(&a.overlap_percent));

    let report = DuplicateTypesReport {
        files_scanned,
        types_analyzed: types.len(),
        min_overlap_percent,
        duplicates,
    };

    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    if report.duplicates.is_empty() { 0 } else { 1 }
}

/// Allow a duplicate type pair by adding to .normalize/duplicate-types-allow
pub fn cmd_allow_duplicate_type(
    root: &Path,
    type1: &str,
    type2: &str,
    reason: Option<&str>,
) -> i32 {
    // Normalize to sorted order
    let (type1, type2) = if type1 < type2 {
        (type1, type2)
    } else {
        (type2, type1)
    };
    let entry = format!("{} {}", type1, type2);

    // Load existing allowlist
    let allowlist_path = root.join(".normalize/duplicate-types-allow");
    let existing_content = std::fs::read_to_string(&allowlist_path).unwrap_or_default();
    let existing_lines: Vec<&str> = existing_content.lines().collect();

    // Check if already exists
    for line in &existing_lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 {
            let (a, b) = if parts[0] < parts[1] {
                (parts[0], parts[1])
            } else {
                (parts[1], parts[0])
            };
            if a == type1 && b == type2 {
                println!("Pair already allowed: {}", entry);
                return 0;
            }
        }
    }

    // Require reason for new entries
    if reason.is_none() {
        eprintln!("Reason required for new type pairs. Use --reason \"...\"");
        return 1;
    }

    // Build new content
    let mut new_lines: Vec<String> = existing_lines.iter().map(|s| s.to_string()).collect();
    if !new_lines.is_empty() && !new_lines.last().is_none_or(|l| l.is_empty()) {
        new_lines.push(String::new());
    }
    if let Some(r) = reason {
        new_lines.push(format!("# {}", r));
    }
    new_lines.push(entry.clone());

    // Ensure .normalize directory exists
    let moss_dir = root.join(".normalize");
    if !moss_dir.exists()
        && let Err(e) = std::fs::create_dir_all(&moss_dir)
    {
        eprintln!("Failed to create .normalize directory: {}", e);
        return 1;
    }

    // Write back
    let new_content = new_lines.join("\n") + "\n";
    if let Err(e) = std::fs::write(&allowlist_path, new_content) {
        eprintln!("Failed to write .normalize/duplicate-types-allow: {}", e);
        return 1;
    }

    println!("Added to .normalize/duplicate-types-allow: {}", entry);
    0
}

/// Flatten nested symbols into a flat list
fn flatten_symbols(sym: &normalize_languages::Symbol) -> Vec<&normalize_languages::Symbol> {
    let mut result = vec![sym];
    for child in &sym.children {
        result.extend(flatten_symbols(child));
    }
    result
}

/// Find a function node at a given line
fn find_function_node(
    tree: &tree_sitter::Tree,
    target_line: usize,
) -> Option<tree_sitter::Node<'_>> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    find_node_at_line_recursive(&mut cursor, target_line)
}

fn find_node_at_line_recursive<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    target_line: usize,
) -> Option<tree_sitter::Node<'a>> {
    loop {
        let node = cursor.node();
        let start = node.start_position().row + 1;

        if start == target_line {
            let kind = node.kind();
            if kind.contains("function")
                || kind.contains("method")
                || kind == "function_definition"
                || kind == "method_definition"
                || kind == "function_item"
                || kind == "function_declaration"
                || kind == "arrow_function"
                || kind == "generator_function"
            {
                return Some(node);
            }
        }

        if cursor.goto_first_child() {
            if let Some(found) = find_node_at_line_recursive(cursor, target_line) {
                return Some(found);
            }
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
    None
}

/// Compute a normalized AST hash for duplicate function detection.
fn compute_function_hash(
    node: &tree_sitter::Node,
    content: &[u8],
    elide_identifiers: bool,
    elide_literals: bool,
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    hash_node_recursive(
        node,
        content,
        &mut hasher,
        elide_identifiers,
        elide_literals,
    );
    hasher.finish()
}

/// Recursively hash a node and its children.
fn hash_node_recursive(
    node: &tree_sitter::Node,
    content: &[u8],
    hasher: &mut impl Hasher,
    elide_identifiers: bool,
    elide_literals: bool,
) {
    let kind = node.kind();

    // Hash the node kind (structure)
    kind.hash(hasher);

    // For leaf nodes, decide whether to hash content
    if node.child_count() == 0 {
        let should_hash = if is_identifier_kind(kind) {
            !elide_identifiers
        } else if is_literal_kind(kind) {
            !elide_literals
        } else {
            // Operators, keywords - their kind is sufficient
            false
        };

        if should_hash {
            let text = &content[node.start_byte()..node.end_byte()];
            text.hash(hasher);
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        hash_node_recursive(&child, content, hasher, elide_identifiers, elide_literals);
    }
}

/// Check if a node kind represents an identifier.
fn is_identifier_kind(kind: &str) -> bool {
    kind == "identifier"
        || kind == "field_identifier"
        || kind == "type_identifier"
        || kind == "property_identifier"
        || kind.ends_with("_identifier")
}

/// Check if a node kind represents a literal value.
fn is_literal_kind(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("integer")
        || kind.contains("float")
        || kind.contains("number")
        || kind.contains("boolean")
        || kind == "true"
        || kind == "false"
        || kind == "nil"
        || kind == "null"
        || kind == "none"
}

// ── Duplicate block detection (subtree-level) ─────────────────────────────────

/// A location of a duplicate block instance
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct DuplicateBlockLocation {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// A group of duplicate blocks
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateBlockGroup {
    #[serde(serialize_with = "serialize_hash")]
    hash: u64,
    pub locations: Vec<DuplicateBlockLocation>,
    pub line_count: usize,
}

/// Duplicate blocks analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateBlocksReport {
    files_scanned: usize,
    blocks_hashed: usize,
    pub groups: Vec<DuplicateBlockGroup>,
    #[serde(skip)]
    show_source: bool,
    #[serde(skip)]
    root: PathBuf,
}

impl OutputFormatter for DuplicateBlocksReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Duplicate Block Detection".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned: {}", self.files_scanned));
        lines.push(format!("Blocks hashed: {}", self.blocks_hashed));
        lines.push(format!("Duplicate groups: {}", self.groups.len()));

        if self.groups.is_empty() {
            lines.push(String::new());
            lines.push("No duplicate blocks detected.".to_string());
            return lines.join("\n");
        }

        lines.push(String::new());

        for (i, group) in self.groups.iter().take(30).enumerate() {
            lines.push(format!(
                "{}. {} lines × {} locations",
                i + 1,
                group.line_count,
                group.locations.len()
            ));
            for loc in &group.locations {
                lines.push(format!(
                    "   {}:{}-{}",
                    loc.file, loc.start_line, loc.end_line
                ));
            }
            if self.show_source
                && let Some(first) = group.locations.first()
            {
                let full_path = self.root.join(&first.file);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let src_lines: Vec<&str> = content.lines().collect();
                    let start = first.start_line.saturating_sub(1);
                    let end = first.end_line.min(src_lines.len());
                    lines.push(String::new());
                    for src_line in &src_lines[start..end] {
                        lines.push(format!("   {}", src_line));
                    }
                }
            }
            lines.push(String::new());
        }

        if self.groups.len() > 30 {
            lines.push(format!("... and {} more groups", self.groups.len() - 30));
        }

        lines.join("\n")
    }
}

/// Walk every node in the tree and hash subtrees at or above min_lines.
fn collect_block_hashes(
    node: &tree_sitter::Node,
    content: &[u8],
    file: &str,
    min_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    out: &mut HashMap<u64, Vec<DuplicateBlockLocation>>,
) {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let line_count = end_line.saturating_sub(start_line) + 1;

    if line_count >= min_lines {
        let hash = compute_function_hash(node, content, elide_identifiers, elide_literals);
        out.entry(hash).or_default().push(DuplicateBlockLocation {
            file: file.to_string(),
            start_line,
            end_line,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_block_hashes(
            &child,
            content,
            file,
            min_lines,
            elide_identifiers,
            elide_literals,
            out,
        );
    }
}

/// After bucketing, suppress groups whose locations are fully contained inside
/// a location from a larger group (in the same file). Returns filtered groups.
fn suppress_contained_blocks(mut groups: Vec<DuplicateBlockGroup>) -> Vec<DuplicateBlockGroup> {
    // Sort largest first so we process parents before children.
    groups.sort_by(|a, b| b.line_count.cmp(&a.line_count));

    // Collect all "taken" ranges per file from already-accepted groups.
    let mut taken: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    let mut result = Vec::new();

    for group in groups {
        let mut kept = Vec::new();
        for loc in &group.locations {
            let ranges = taken.entry(loc.file.clone()).or_default();
            let contained = ranges
                .iter()
                .any(|&(s, e)| s <= loc.start_line && loc.end_line <= e);
            if !contained {
                kept.push(loc.clone());
            }
        }
        if kept.len() >= 2 {
            // Register these locations as taken.
            for loc in &kept {
                taken
                    .entry(loc.file.clone())
                    .or_default()
                    .push((loc.start_line, loc.end_line));
            }
            result.push(DuplicateBlockGroup {
                hash: group.hash,
                locations: kept,
                line_count: group.line_count,
            });
        }
    }

    result
}

pub fn cmd_duplicate_blocks(
    root: &Path,
    min_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    show_source: bool,
    format: &crate::output::OutputFormat,
    filter: Option<&Filter>,
) -> i32 {
    let mut hash_groups: HashMap<u64, Vec<DuplicateBlockLocation>> = HashMap::new();
    let mut files_scanned = 0usize;
    let mut blocks_hashed = 0usize;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();

        if let Some(f) = filter {
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if !f.matches(rel_path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        let tree = match crate::parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        files_scanned += 1;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let before = hash_groups.values().map(|v| v.len()).sum::<usize>();
        collect_block_hashes(
            &tree.root_node(),
            content.as_bytes(),
            &rel_path,
            min_lines,
            elide_identifiers,
            elide_literals,
            &mut hash_groups,
        );
        let after = hash_groups.values().map(|v| v.len()).sum::<usize>();
        blocks_hashed += after - before;
    }

    let groups_raw: Vec<DuplicateBlockGroup> = hash_groups
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .map(|(hash, locations)| {
            let line_count = locations
                .first()
                .map(|l| l.end_line.saturating_sub(l.start_line) + 1)
                .unwrap_or(0);
            DuplicateBlockGroup {
                hash,
                locations,
                line_count,
            }
        })
        .collect();

    let groups = suppress_contained_blocks(groups_raw);

    let report = DuplicateBlocksReport {
        files_scanned,
        blocks_hashed,
        groups,
        show_source,
        root: root.to_path_buf(),
    };

    report.print(format);
    0
}

// ── Fuzzy / partial clone detection (MinHash LSH) ─────────────────────────────

const MINHASH_N: usize = 128;
const SHINGLE_K: usize = 3;
const LSH_BANDS: usize = 32; // 32 bands × 4 rows → good recall at ≥0.7 similarity
const LSH_ROWS: usize = 4; // MINHASH_N / LSH_BANDS

/// A pair of similar (but not necessarily identical) blocks.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimilarBlockPair {
    pub location_a: DuplicateBlockLocation,
    pub location_b: DuplicateBlockLocation,
    /// Estimated Jaccard similarity of their AST token shingles (0.0–1.0)
    pub similarity: f64,
    pub line_count: usize,
}

/// Report from fuzzy block similarity detection.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimilarBlocksReport {
    files_scanned: usize,
    blocks_analyzed: usize,
    threshold: f64,
    pub pairs: Vec<SimilarBlockPair>,
    #[serde(skip)]
    show_source: bool,
    #[serde(skip)]
    root: PathBuf,
}

impl OutputFormatter for SimilarBlocksReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Similar Block Detection (fuzzy)".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned:   {}", self.files_scanned));
        lines.push(format!("Blocks analyzed: {}", self.blocks_analyzed));
        lines.push(format!("Threshold:       {:.0}%", self.threshold * 100.0));
        lines.push(format!("Similar pairs:   {}", self.pairs.len()));

        if self.pairs.is_empty() {
            lines.push(String::new());
            lines.push("No similar blocks detected.".to_string());
            return lines.join("\n");
        }

        lines.push(String::new());

        for (i, pair) in self.pairs.iter().take(30).enumerate() {
            lines.push(format!(
                "{}. {:.0}% similar  ({} lines)",
                i + 1,
                pair.similarity * 100.0,
                pair.line_count,
            ));
            lines.push(format!(
                "   {}:{}-{}",
                pair.location_a.file, pair.location_a.start_line, pair.location_a.end_line
            ));
            lines.push(format!(
                "   {}:{}-{}",
                pair.location_b.file, pair.location_b.start_line, pair.location_b.end_line
            ));

            if self.show_source {
                for loc in [&pair.location_a, &pair.location_b] {
                    let full_path = self.root.join(&loc.file);
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        let src_lines: Vec<&str> = content.lines().collect();
                        let start = loc.start_line.saturating_sub(1);
                        let end = loc.end_line.min(src_lines.len());
                        lines.push(String::new());
                        lines.push(format!("   --- {} ---", loc.file));
                        for src_line in &src_lines[start..end] {
                            lines.push(format!("   {}", src_line));
                        }
                    }
                }
            }
            lines.push(String::new());
        }

        if self.pairs.len() > 30 {
            lines.push(format!("... and {} more pairs", self.pairs.len() - 30));
        }

        lines.join("\n")
    }
}

/// Serialize an AST subtree to a flat token sequence for shingling.
/// Returns true for node kinds that represent a block/body — used by skeleton
/// mode to replace the entire subtree with a `<body>` placeholder token.
fn is_body_kind(kind: &str) -> bool {
    // Exact matches for common block/body node names across languages.
    matches!(
        kind,
        "block"                  // Rust, Go, many others
        | "body"                 // Python, Kotlin
        | "statement_block"      // JavaScript, TypeScript
        | "compound_statement"   // C, C++, Bash
        | "declaration_list"     // C/C++ struct/union body
        | "field_declaration_list" // Rust struct body
        | "enum_body"            // Java, Kotlin
        | "class_body"           // Java, Kotlin, TypeScript
        | "interface_body"       // Java
        | "object_body"          // Kotlin
        | "do_block"             // Ruby
        | "begin_block"          // Ruby
        | "block_body" // generic
    ) || kind.ends_with("_body")
        || kind.ends_with("_block")
        || kind.ends_with("_list") && kind.contains("statement")
}

/// Hash token for skeleton body placeholder — a fixed sentinel value.
const BODY_PLACEHOLDER: u64 = 0xb0d7_b0d7_b0d7_b0d7;

fn serialize_subtree_tokens(
    node: &tree_sitter::Node,
    content: &[u8],
    elide_identifiers: bool,
    elide_literals: bool,
    skeleton: bool,
    out: &mut Vec<u64>,
) {
    use std::collections::hash_map::DefaultHasher;
    let kind = node.kind();

    // In skeleton mode, replace body/block subtrees with a fixed placeholder.
    if skeleton && node.child_count() > 0 && is_body_kind(kind) {
        out.push(BODY_PLACEHOLDER);
        return;
    }

    let mut h = DefaultHasher::new();
    kind.hash(&mut h);

    if node.child_count() == 0 {
        let should_include = if is_identifier_kind(kind) {
            !elide_identifiers
        } else if is_literal_kind(kind) {
            !elide_literals
        } else {
            false
        };
        if should_include {
            let text = &content[node.start_byte()..node.end_byte()];
            text.hash(&mut h);
        }
    }
    out.push(h.finish());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        serialize_subtree_tokens(
            &child,
            content,
            elide_identifiers,
            elide_literals,
            skeleton,
            out,
        );
    }
}

/// Simple universal hash for MinHash: mixes x with a per-function seed.
#[inline]
fn minhash_hash(x: u64, seed: u64) -> u64 {
    let a = 6364136223846793005u64.wrapping_add(seed.wrapping_mul(2654435761));
    let b = 1442695040888963407u64.wrapping_add(seed.wrapping_mul(1013904223));
    a.wrapping_mul(x).wrapping_add(b)
}

/// Compute a MinHash signature over k-shingles of the token sequence.
fn compute_minhash(tokens: &[u64]) -> [u64; MINHASH_N] {
    let mut sig = [u64::MAX; MINHASH_N];
    if tokens.len() < SHINGLE_K {
        return sig;
    }
    for window in tokens.windows(SHINGLE_K) {
        // Hash the shingle into a single u64.
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        window.hash(&mut h);
        let shingle_hash = h.finish();

        for (i, slot) in sig.iter_mut().enumerate() {
            let v = minhash_hash(shingle_hash, i as u64);
            if v < *slot {
                *slot = v;
            }
        }
    }
    sig
}

/// Estimate Jaccard similarity from two MinHash signatures.
fn jaccard_estimate(a: &[u64; MINHASH_N], b: &[u64; MINHASH_N]) -> f64 {
    let matches = a.iter().zip(b.iter()).filter(|(x, y)| x == y).count();
    matches as f64 / MINHASH_N as f64
}

/// Hash one LSH band of a signature.
fn lsh_band_hash(sig: &[u64; MINHASH_N], band: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let start = band * LSH_ROWS;
    let mut h = DefaultHasher::new();
    band.hash(&mut h);
    for v in &sig[start..start + LSH_ROWS] {
        v.hash(&mut h);
    }
    h.finish()
}

/// Collect all subtrees above min_lines, returning (location, minhash signature).
#[allow(clippy::too_many_arguments)]
fn collect_block_signatures(
    node: &tree_sitter::Node,
    content: &[u8],
    file: &str,
    min_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    skeleton: bool,
    out: &mut Vec<(DuplicateBlockLocation, [u64; MINHASH_N])>,
) {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let line_count = end_line.saturating_sub(start_line) + 1;

    if line_count >= min_lines {
        let mut tokens = Vec::new();
        serialize_subtree_tokens(
            node,
            content,
            elide_identifiers,
            elide_literals,
            skeleton,
            &mut tokens,
        );
        // In skeleton mode a complex function can reduce to very few tokens,
        // or to a long repetitive sequence (e.g. many match arms all elided).
        // Both cases produce false positives — filter them out.
        let min_tokens = if skeleton { SHINGLE_K * 4 } else { SHINGLE_K };
        let skip = if tokens.len() < min_tokens {
            true
        } else if skeleton {
            // Require at least 30% unique tokens: repetitive skeletons
            // (long match statements, uniform loop bodies) are not useful signal.
            let unique = tokens
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len();
            (unique as f64 / tokens.len() as f64) < 0.3
        } else {
            false
        };

        if !skip {
            let sig = compute_minhash(&tokens);
            out.push((
                DuplicateBlockLocation {
                    file: file.to_string(),
                    start_line,
                    end_line,
                },
                sig,
            ));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_block_signatures(
            &child,
            content,
            file,
            min_lines,
            elide_identifiers,
            elide_literals,
            skeleton,
            out,
        );
    }
}

/// Suppress overlapping pairs: if two pairs involve the same two files and
/// both locations significantly overlap, keep only the largest (first seen,
/// since pairs are pre-sorted by similarity desc then size desc).
fn suppress_overlapping_pairs(pairs: Vec<SimilarBlockPair>) -> Vec<SimilarBlockPair> {
    // Track accepted ranges per file.
    let mut taken: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut result = Vec::new();

    for pair in pairs {
        let taken_a = taken.entry(pair.location_a.file.clone()).or_default();
        let overlaps_a = taken_a.iter().any(|&(s, e)| {
            overlap_ratio(s, e, pair.location_a.start_line, pair.location_a.end_line) > 0.5
        });

        let taken_b = taken.entry(pair.location_b.file.clone()).or_default();
        let overlaps_b = taken_b.iter().any(|&(s, e)| {
            overlap_ratio(s, e, pair.location_b.start_line, pair.location_b.end_line) > 0.5
        });

        if overlaps_a && overlaps_b {
            continue;
        }

        taken
            .entry(pair.location_a.file.clone())
            .or_default()
            .push((pair.location_a.start_line, pair.location_a.end_line));
        taken
            .entry(pair.location_b.file.clone())
            .or_default()
            .push((pair.location_b.start_line, pair.location_b.end_line));

        result.push(pair);
    }

    result
}

fn overlap_ratio(s1: usize, e1: usize, s2: usize, e2: usize) -> f64 {
    let overlap_start = s1.max(s2);
    let overlap_end = e1.min(e2);
    if overlap_end < overlap_start {
        return 0.0;
    }
    let overlap = (overlap_end - overlap_start + 1) as f64;
    let union = (e1.max(e2) - s1.min(s2) + 1) as f64;
    overlap / union
}

pub struct SimilarBlocksConfig<'a> {
    pub root: &'a Path,
    pub min_lines: usize,
    pub similarity: f64,
    pub elide_identifiers: bool,
    pub elide_literals: bool,
    pub skeleton: bool,
    pub show_source: bool,
    pub format: &'a crate::output::OutputFormat,
    pub filter: Option<&'a Filter>,
}

pub fn cmd_similar_blocks(cfg: SimilarBlocksConfig<'_>) -> i32 {
    let SimilarBlocksConfig {
        root,
        min_lines,
        similarity,
        elide_identifiers,
        elide_literals,
        skeleton,
        show_source,
        format,
        filter,
    } = cfg;
    let mut all_blocks: Vec<(DuplicateBlockLocation, [u64; MINHASH_N])> = Vec::new();
    let mut files_scanned = 0usize;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();

        if let Some(f) = filter {
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if !f.matches(rel_path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        let tree = match crate::parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        files_scanned += 1;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        collect_block_signatures(
            &tree.root_node(),
            content.as_bytes(),
            &rel_path,
            min_lines,
            elide_identifiers,
            elide_literals,
            skeleton,
            &mut all_blocks,
        );
    }

    let blocks_analyzed = all_blocks.len();

    // LSH: for each band, bucket block indices by their band hash.
    let mut band_buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, (_, sig)) in all_blocks.iter().enumerate() {
        for band in 0..LSH_BANDS {
            let bh = lsh_band_hash(sig, band);
            // Mix band id into bucket key to avoid cross-band collisions.
            let key = bh.wrapping_add((band as u64).wrapping_mul(0x9e3779b97f4a7c15));
            band_buckets.entry(key).or_default().push(idx);
        }
    }

    // Collect candidate pairs (deduplicated).
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for bucket in band_buckets.values() {
        if bucket.len() < 2 {
            continue;
        }
        for i in 0..bucket.len() {
            for j in i + 1..bucket.len() {
                let (a, b) = (bucket[i].min(bucket[j]), bucket[i].max(bucket[j]));
                if seen.insert((a, b)) {
                    candidates.push((a, b));
                }
            }
        }
    }

    // Score candidates, filter by threshold.
    let mut pairs: Vec<SimilarBlockPair> = candidates
        .into_iter()
        .filter_map(|(i, j)| {
            let (loc_a, sig_a) = &all_blocks[i];
            let (loc_b, sig_b) = &all_blocks[j];

            // Skip same-location pairs (a subtree always matches itself).
            if loc_a.file == loc_b.file
                && loc_a.start_line == loc_b.start_line
                && loc_a.end_line == loc_b.end_line
            {
                return None;
            }

            // Skip containment pairs: one subtree fully contains the other in
            // the same file. These are parent/child AST nodes — not useful signal.
            if loc_a.file == loc_b.file {
                let a_contains_b =
                    loc_a.start_line <= loc_b.start_line && loc_b.end_line <= loc_a.end_line;
                let b_contains_a =
                    loc_b.start_line <= loc_a.start_line && loc_a.end_line <= loc_b.end_line;
                if a_contains_b || b_contains_a {
                    return None;
                }
            }

            // Skip pairs with very different sizes. In skeleton mode bodies are
            // elided so size differences are expected — relax from 0.5 to 0.1
            // (allow up to 10× difference) rather than disabling entirely.
            {
                let len_a = loc_a.end_line.saturating_sub(loc_a.start_line) + 1;
                let len_b = loc_b.end_line.saturating_sub(loc_b.start_line) + 1;
                let ratio = len_a.min(len_b) as f64 / len_a.max(len_b) as f64;
                let min_ratio = if skeleton { 0.2 } else { 0.5 };
                if ratio < min_ratio {
                    return None;
                }
            }

            let sim = jaccard_estimate(sig_a, sig_b);
            if sim < similarity {
                return None;
            }

            let line_count = loc_a
                .end_line
                .saturating_sub(loc_a.start_line)
                .max(loc_b.end_line.saturating_sub(loc_b.start_line))
                + 1;

            Some(SimilarBlockPair {
                location_a: loc_a.clone(),
                location_b: loc_b.clone(),
                similarity: sim,
                line_count,
            })
        })
        .collect();

    pairs.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.line_count.cmp(&a.line_count))
    });

    let pairs = suppress_overlapping_pairs(pairs);

    let report = SimilarBlocksReport {
        files_scanned,
        blocks_analyzed,
        threshold: similarity,
        pairs,
        show_source,
        root: root.to_path_buf(),
    };

    report.print(format);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_duplicate_functions_allowlist_empty() {
        let tmp = tempdir().unwrap();
        let allowlist = load_duplicate_functions_allowlist(tmp.path());
        assert!(allowlist.is_empty());
    }

    #[test]
    fn test_suppress_contained_blocks_removes_children() {
        // Parent group: 20 lines, child group: 5 lines (contained within parent)
        let parent = DuplicateBlockGroup {
            hash: 1,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 1,
                    end_line: 20,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 1,
                    end_line: 20,
                },
            ],
            line_count: 20,
        };
        let child = DuplicateBlockGroup {
            hash: 2,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 5,
                    end_line: 10,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 5,
                    end_line: 10,
                },
            ],
            line_count: 6,
        };
        let result = suppress_contained_blocks(vec![parent, child]);
        assert_eq!(result.len(), 1, "child group should be suppressed");
        assert_eq!(result[0].line_count, 20);
    }

    #[test]
    fn test_suppress_contained_blocks_keeps_non_overlapping() {
        let a = DuplicateBlockGroup {
            hash: 1,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 1,
                    end_line: 10,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 1,
                    end_line: 10,
                },
            ],
            line_count: 10,
        };
        let b = DuplicateBlockGroup {
            hash: 2,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 20,
                    end_line: 30,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 20,
                    end_line: 30,
                },
            ],
            line_count: 11,
        };
        let result = suppress_contained_blocks(vec![a, b]);
        assert_eq!(
            result.len(),
            2,
            "non-overlapping groups should both survive"
        );
    }
}
