//! Initialize normalize in a project directory.

use clap::Args;
use std::fs;
use std::path::Path;

#[derive(Args, Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InitArgs {
    /// Index the codebase after initialization
    #[arg(long)]
    #[serde(default)]
    pub index: bool,
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(InitArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run init command.
pub fn run(args: InitArgs, input_schema: bool, params_json: Option<&str>) -> i32 {
    if input_schema {
        print_input_schema();
        return 0;
    }
    // Override args with --params-json if provided
    let args = match params_json {
        Some(json) => match serde_json::from_str(json) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("error: invalid --params-json: {}", e);
                return 1;
            }
        },
        None => args,
    };
    let root = std::env::current_dir().unwrap();
    cmd_init(&root, args.index)
}

/// Common TODO file names to detect
const TODO_CANDIDATES: &[&str] = &[
    "TODO.md",
    "TASKS.md",
    "TODO.txt",
    "TASKS.txt",
    "TODO",
    "TASKS",
];

fn cmd_init(root: &Path, do_index: bool) -> i32 {
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

    // 2. Detect TODO files for sigil config
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

    0
}

/// Detect TODO files in the project root.
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
        let result = cmd_init(tmp.path(), false);
        assert_eq!(result, 0);
        assert!(tmp.path().join(".normalize").exists());
        assert!(tmp.path().join(".normalize/config.toml").exists());
    }

    #[test]
    fn test_init_idempotent() {
        let tmp = tempdir().unwrap();
        let result1 = cmd_init(tmp.path(), false);
        let result2 = cmd_init(tmp.path(), false);
        assert_eq!(result1, 0);
        assert_eq!(result2, 0);
    }

    #[test]
    fn test_init_updates_gitignore() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();

        cmd_init(tmp.path(), false);

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".normalize/*"));
        assert!(content.contains("!.normalize/config.toml"));
    }

    #[test]
    fn test_init_skips_commented_entries() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "# .normalize\n").unwrap();

        cmd_init(tmp.path(), false);

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

        cmd_init(tmp.path(), false);

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

        cmd_init(tmp.path(), false);

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        assert!(config.contains("[aliases]"));
        assert!(config.contains("TODO.md"));
        assert!(config.contains("TASKS.md"));
    }

    #[test]
    fn test_init_no_todo_files() {
        let tmp = tempdir().unwrap();

        cmd_init(tmp.path(), false);

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        // Should not have aliases section if no TODO files found
        assert!(!config.contains("[aliases]"));
    }
}
