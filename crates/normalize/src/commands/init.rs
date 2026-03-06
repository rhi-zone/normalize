//! Initialize normalize in a project directory.

use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

/// Common task-tracker file names to detect
const TODO_CANDIDATES: &[&str] = &[
    "TODO.md",
    "TASKS.md",
    "TODO.txt",
    "TASKS.txt",
    "TODO",
    "TASKS",
];

pub fn cmd_init(root: &Path, do_index: bool, setup: bool) -> i32 {
    let mut changes = Vec::new();

    // 1. Create .normalize directory if needed
    let moss_dir = root.join(".normalize");
    if !moss_dir.exists() {
        if let Err(e) = fs::create_dir_all(&moss_dir) {
            eprintln!("Failed to create .normalize directory: {}", e);
            return 1;
        }
        changes.push("Created .normalize/".to_string());
    }

    // 2. Detect task-tracking files for sigil config
    let todo_files = detect_todo_files(root);

    // 3. Create or update config.toml
    let config_path = moss_dir.join("config.toml");
    if !config_path.exists() {
        let aliases_section = if todo_files.is_empty() {
            String::new()
        } else {
            let files_str = todo_files
                .iter()
                .map(|f| format!("\"{}\"", f))
                .collect::<Vec<_>>()
                .join(", ");
            format!("\n[aliases]\ntodo = [{}]\n", files_str)
        };

        let default_config = format!(
            r#"# Normalize configuration
# See: https://github.com/rhi-zone/normalize

[daemon]
# enabled = true
# auto_start = true

[analyze]
# clones = true

# [analyze.weights]
# health = 1.0
# complexity = 0.5
# security = 2.0
# clones = 0.3
{}"#,
            aliases_section
        );
        if let Err(e) = fs::write(&config_path, default_config) {
            eprintln!("Failed to create config.toml: {}", e);
            return 1;
        }
        changes.push("Created .normalize/config.toml".to_string());
        for f in &todo_files {
            changes.push(format!("Detected TODO file: {}", f));
        }
    }

    // 3. Update .gitignore if needed
    let gitignore_path = root.join(".gitignore");
    let gitignore_changes = update_gitignore(&gitignore_path);
    changes.extend(gitignore_changes);

    // 4. Report changes
    if changes.is_empty() {
        println!("Already initialized.");
    } else {
        println!("Initialized normalize:");
        for change in &changes {
            println!("  {}", change);
        }
    }

    // 5. Optionally index
    if do_index {
        println!("\nIndexing codebase...");
        // normalize-syntax-allow: rust/unwrap-in-impl - Runtime::new() only fails on OS resource exhaustion
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut idx = match rt.block_on(crate::index::open(root)) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("Failed to open index: {}", e);
                return 1;
            }
        };
        match rt.block_on(idx.refresh()) {
            Ok(count) => println!("Indexed {} files.", count),
            Err(e) => {
                eprintln!("Failed to index: {}", e);
                return 1;
            }
        }
    }

    // 6. Optionally run the interactive setup wizard
    if setup {
        println!();
        return cmd_setup_wizard(root);
    }

    0
}

/// Interactive rule setup wizard.
///
/// Runs all rules against the codebase, groups violations by rule, and walks the user
/// through each rule that has violations — showing examples and prompting enable/disable.
pub fn cmd_setup_wizard(root: &Path) -> i32 {
    use crate::commands::rules::{RuleType, run_rules_report};
    use normalize_facts_rules_interpret as interpret;
    use normalize_syntax_rules;
    use std::collections::HashMap;

    let use_colors = io::stdout().is_terminal();

    println!("Rule Setup Wizard");
    println!("=================");
    println!("Running all rules against the codebase...\n");

    let config = crate::config::NormalizeConfig::load(root);

    // Load rule metadata for descriptions
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
    let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);

    // Build map: rule_id -> (description, severity, enabled, type)
    let mut rule_meta: HashMap<String, RuleMeta> = HashMap::new();
    for r in &syntax_rules {
        rule_meta.insert(
            r.id.clone(),
            RuleMeta {
                message: r.message.clone(),
                severity: r.severity.to_string(),
                enabled: r.enabled,
                rule_type: "syntax",
            },
        );
    }
    for r in &fact_rules {
        rule_meta.insert(
            r.id.clone(),
            RuleMeta {
                message: r.message.clone(),
                severity: r.severity.to_string(),
                enabled: r.enabled,
                rule_type: "fact",
            },
        );
    }

    // Run all rules to collect violations
    let report = run_rules_report(root, None, None, &RuleType::All, &[], &config);

    // Group issues by rule_id
    let mut by_rule: HashMap<String, Vec<normalize_output::diagnostics::Issue>> = HashMap::new();
    for issue in &report.issues {
        by_rule
            .entry(issue.rule_id.clone())
            .or_default()
            .push(issue.clone());
    }

    // Sort rules: by violation count desc, then alphabetically
    let mut rules_with_violations: Vec<(String, Vec<normalize_output::diagnostics::Issue>)> =
        by_rule.into_iter().collect();
    rules_with_violations.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    if rules_with_violations.is_empty() {
        println!("No violations found — all rules pass on this codebase.");
        println!("Use `normalize rules list` to see available rules.");
        return 0;
    }

    let total_rules = rules_with_violations.len();
    println!(
        "Found violations from {} rules across {} files checked.\n",
        total_rules, report.files_checked
    );
    println!("For each rule, choose:");
    println!("  [e]nable   — enable this rule (violations become errors/warnings)");
    println!("  [d]isable  — disable this rule (suppress violations)");
    println!("  [s]kip     — keep current setting (default, press Enter)");
    println!("  [q]uit     — stop here and keep remaining rules unchanged\n");

    let gray = nu_ansi_term::Color::DarkGray;
    let bold = nu_ansi_term::Style::new().bold();

    let stdin = io::stdin();
    let mut changed = 0;

    for (i, (rule_id, issues)) in rules_with_violations.iter().enumerate() {
        let meta = rule_meta.get(rule_id);
        let enabled = meta.is_some_and(|m| m.enabled);
        let severity = meta.map_or("info", |m| m.severity.as_str());
        let rule_type = meta.map_or("?", |m| m.rule_type);
        let description = meta.map_or("", |m| m.message.as_str());

        // Rule header
        let sev_colored = paint_severity(severity, use_colors);
        let state = if enabled { "enabled" } else { "disabled" };
        let state_str = if use_colors {
            if enabled {
                nu_ansi_term::Color::Green.paint(state).to_string()
            } else {
                gray.paint(state).to_string()
            }
        } else {
            state.to_string()
        };

        println!(
            "─── ({}/{}) {} [{}] {} ───",
            i + 1,
            total_rules,
            if use_colors {
                bold.paint(rule_id).to_string()
            } else {
                rule_id.clone()
            },
            rule_type,
            state_str
        );
        println!(
            "    Severity: {}  |  {} violations",
            sev_colored,
            issues.len()
        );
        if !description.is_empty() {
            println!("    {}", description);
        }
        println!();

        // Show up to 5 example violations
        let sample = issues.iter().take(5);
        for issue in sample {
            let location = match issue.line {
                Some(line) => format!("{}:{}", issue.file, line),
                None => issue.file.clone(),
            };
            let loc_str = if use_colors {
                gray.paint(&location).to_string()
            } else {
                location
            };
            println!("    {} — {}", loc_str, issue.message);
        }
        if issues.len() > 5 {
            let more = issues.len() - 5;
            let more_str = format!("    ... and {} more", more);
            if use_colors {
                println!("{}", gray.paint(&more_str));
            } else {
                println!("{}", more_str);
            }
        }
        println!();

        // Prompt
        let prompt = format!("    [e]nable / [d]isable / [s]kip (current: {}) > ", state);
        print!("{}", prompt);
        io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            eprintln!("Failed to read input");
            return 1;
        }

        match line.trim().to_lowercase().as_str() {
            "e" | "enable" => {
                if !enabled {
                    match crate::commands::rules::cmd_enable_disable_service(
                        Some(root.to_str().unwrap_or(".")),
                        rule_id,
                        true,
                        false,
                    ) {
                        Ok(_) => {
                            println!("  → Enabled {}", rule_id);
                            changed += 1;
                        }
                        Err(e) => eprintln!("  Error enabling {}: {}", rule_id, e),
                    }
                } else {
                    println!("  → Already enabled");
                }
            }
            "d" | "disable" => {
                if enabled {
                    match crate::commands::rules::cmd_enable_disable_service(
                        Some(root.to_str().unwrap_or(".")),
                        rule_id,
                        false,
                        false,
                    ) {
                        Ok(_) => {
                            println!("  → Disabled {}", rule_id);
                            changed += 1;
                        }
                        Err(e) => eprintln!("  Error disabling {}: {}", rule_id, e),
                    }
                } else {
                    println!("  → Already disabled");
                }
            }
            "q" | "quit" => {
                println!("\nStopped at rule {}/{}", i + 1, total_rules);
                break;
            }
            _ => {
                // skip or empty
                println!("  → Skipped");
            }
        }
        println!();
    }

    if changed > 0 {
        println!(
            "Setup complete. {} rule(s) updated in .normalize/config.toml",
            changed
        );
        println!("Run `normalize rules run` to see remaining violations.");
    } else {
        println!("Setup complete. No changes made.");
    }

    0
}

struct RuleMeta {
    message: String,
    severity: String,
    enabled: bool,
    rule_type: &'static str,
}

fn paint_severity(severity: &str, use_colors: bool) -> String {
    if !use_colors {
        return severity.to_string();
    }
    match severity {
        "error" => nu_ansi_term::Color::Red.paint(severity).to_string(),
        "warning" => nu_ansi_term::Color::Yellow.paint(severity).to_string(),
        "info" => nu_ansi_term::Color::Cyan.paint(severity).to_string(),
        "hint" => nu_ansi_term::Color::DarkGray.paint(severity).to_string(),
        _ => severity.to_string(),
    }
}

/// Detect task-tracking files (TODO.md, TASKS.md, etc.) in the project root.
fn detect_todo_files(root: &Path) -> Vec<String> {
    TODO_CANDIDATES
        .iter()
        .filter(|name| root.join(name).exists())
        .map(|s| s.to_string())
        .collect()
}

/// Entries we want in .gitignore
/// - .normalize/* ignores root .normalize/ contents (patterns with / only match at root)
/// - !.normalize/... un-ignores specific files (works because /* ignores contents, not the dir)
///   NOTE: We omit **/.normalize/ because it would block un-ignore patterns entirely.
const GITIGNORE_ENTRIES: &[&str] = &[
    ".normalize/*",
    "!.normalize/config.toml",
    "!.normalize/duplicate-functions-allow",
    "!.normalize/duplicate-types-allow",
    "!.normalize/hotspots-allow",
    "!.normalize/large-files-allow",
    "!.normalize/memory/",
];

/// Update .gitignore with normalize entries. Returns list of changes made.
fn update_gitignore(path: &Path) -> Vec<String> {
    let mut changes = Vec::new();

    // Read existing content
    let content = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();

    // Check which entries are missing and find best insertion point
    let mut to_add = Vec::new();
    let mut insert_after: Option<usize> = None; // Line index to insert after

    for entry in GITIGNORE_ENTRIES {
        match find_entry(&lines, entry) {
            EntryStatus::Missing => {
                to_add.push(*entry);
            }
            EntryStatus::CommentedOut(line_num) => {
                eprintln!(
                    "Note: '{}' is commented out in .gitignore (line {}), skipping",
                    entry,
                    line_num + 1
                );
            }
            EntryStatus::Present(line_num) => {
                // Track where existing normalize entries are for best insertion point
                insert_after = Some(insert_after.map_or(line_num, |prev| prev.max(line_num)));
            }
        }
    }

    if to_add.is_empty() {
        return changes;
    }

    // Build new content
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    if let Some(idx) = insert_after {
        // Insert near existing normalize entries
        let insert_pos = idx + 1;
        for (i, entry) in to_add.iter().enumerate() {
            new_lines.insert(insert_pos + i, entry.to_string());
            changes.push(format!("Added '{}' to .gitignore", entry));
        }
    } else {
        // Append at end with header
        if !new_lines.is_empty() && !new_lines.last().is_none_or(|l| l.is_empty()) {
            new_lines.push(String::new());
        }
        new_lines.push("# Normalize".to_string());
        for entry in &to_add {
            new_lines.push(entry.to_string());
            changes.push(format!("Added '{}' to .gitignore", entry));
        }
    }

    let new_content = new_lines.join("\n") + "\n";
    if let Err(e) = fs::write(path, new_content) {
        eprintln!("Failed to update .gitignore: {}", e);
        return Vec::new();
    }

    changes
}

enum EntryStatus {
    Missing,
    Present(usize),
    CommentedOut(usize),
}

/// Check if an entry exists in gitignore lines.
fn find_entry(lines: &[&str], pattern: &str) -> EntryStatus {
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Exact match
        if trimmed == pattern {
            return EntryStatus::Present(i);
        }

        // Check if commented version exists
        if trimmed.starts_with('#') {
            let uncommented = trimmed.trim_start_matches('#').trim();
            if uncommented == pattern {
                return EntryStatus::CommentedOut(i);
            }
        }
    }
    EntryStatus::Missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_init_creates_moss_dir() {
        let tmp = tempdir().unwrap();
        let result = cmd_init(tmp.path(), false, false);
        assert_eq!(result, 0);
        assert!(tmp.path().join(".normalize").exists());
        assert!(tmp.path().join(".normalize/config.toml").exists());
    }

    #[test]
    fn test_init_idempotent() {
        let tmp = tempdir().unwrap();
        let result1 = cmd_init(tmp.path(), false, false);
        let result2 = cmd_init(tmp.path(), false, false);
        assert_eq!(result1, 0);
        assert_eq!(result2, 0);
    }

    #[test]
    fn test_init_updates_gitignore() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();

        cmd_init(tmp.path(), false, false);

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".normalize/*"));
        assert!(content.contains("!.normalize/config.toml"));
    }

    #[test]
    fn test_init_skips_commented_entries() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "# .normalize\n").unwrap();

        cmd_init(tmp.path(), false, false);

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // .normalize should remain commented (not added as active entry)
        assert!(lines.iter().any(|l| l.trim() == "# .normalize"));
        assert!(!lines.iter().any(|l| l.trim() == ".normalize"));

        // But negation entries should still be added
        assert!(lines.iter().any(|l| l.trim() == "!.normalize/config.toml"));
        assert!(
            lines
                .iter()
                .any(|l| l.trim() == "!.normalize/duplicate-functions-allow")
        );
        assert!(
            lines
                .iter()
                .any(|l| l.trim() == "!.normalize/duplicate-types-allow")
        );
    }

    #[test]
    fn test_init_inserts_near_existing() {
        let tmp = tempdir().unwrap();
        // Existing .gitignore already has .normalize/*
        fs::write(
            tmp.path().join(".gitignore"),
            "node_modules\n.normalize/*\nother_stuff\n",
        )
        .unwrap();

        cmd_init(tmp.path(), false, false);

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // New entries should be inserted after .normalize/*, before other_stuff
        let moss_idx = lines.iter().position(|l| *l == ".normalize/*").unwrap();
        let other_idx = lines.iter().position(|l| *l == "other_stuff").unwrap();
        let config_idx = lines
            .iter()
            .position(|l| *l == "!.normalize/config.toml")
            .unwrap();

        // All new entries should be between .normalize/* and other_stuff
        assert!(config_idx > moss_idx, "config should be after .normalize/*");
        assert!(
            config_idx < other_idx,
            "config should be before other_stuff"
        );
    }

    #[test]
    fn test_init_detects_todo_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("TODO.md"), "# TODO\n").unwrap();
        fs::write(tmp.path().join("TASKS.md"), "# Tasks\n").unwrap();

        cmd_init(tmp.path(), false, false);

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        assert!(config.contains("[aliases]"));
        assert!(config.contains("TODO.md"));
        assert!(config.contains("TASKS.md"));
    }

    #[test]
    fn test_init_no_todo_files() {
        let tmp = tempdir().unwrap();

        cmd_init(tmp.path(), false, false);

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        // Should not have aliases section if no todo-tracking files found
        assert!(!config.contains("[aliases]"));
    }
}
