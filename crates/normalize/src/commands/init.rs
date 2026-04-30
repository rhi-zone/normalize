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

/// Scratch directories that are commonly tracked (not in `.gitignore`) but
/// shouldn't be linted as part of this project. `init` detects which of these
/// exist in the project and offers to add them to `[walk] exclude`.
///
/// Build/temp directories from language conventions (`target/`, `node_modules/`,
/// `.venv/`, `__pycache__/`, etc.) are deliberately *not* listed here — they
/// are conventionally gitignored, which the walker respects automatically.
/// Only commonly-tracked-but-not-lintable dirs need explicit exclusion.
///
/// Per `CLAUDE.md`, this list of third-party-tool paths lives in `init.rs`
/// (the integration layer), never in `normalize-rules-config` or other
/// rule-engine crates.
const SCRATCH_DIRS: &[(&str, &str)] = &[(
    ".claude/worktrees/",
    "Claude Code agent worktrees (ephemeral scratch — full repo copies)",
)];

pub async fn run_init(root: &Path, do_index: bool, setup: bool) -> i32 {
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
        // Start from the bootstrap (opinionated) config, then layer on
        // project-specific detections.
        let mut excludes: Vec<String> = vec![".git/".to_string()];

        let detected = detect_scratch_dirs(root);
        let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
        let accepted: Vec<&(&str, &str)> = if interactive && !detected.is_empty() {
            prompt_scratch_dirs(&detected)
        } else {
            // Non-interactive: include all detected entries by default.
            detected.iter().collect()
        };
        for (dir, _) in &accepted {
            excludes.push((*dir).to_string());
            changes.push(format!("Detected scratch dir: {} → [walk] exclude", dir));
        }

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

        let exclude_lines = excludes
            .iter()
            .map(|s| format!("    \"{}\",", s))
            .collect::<Vec<_>>()
            .join("\n");
        let walk_section = format!(
            r#"
[walk]
# Project-wide path exclusions (gitignore-style; affects walker AND index).
exclude = [
{}
]
"#,
            exclude_lines
        );

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
{}{}"#,
            walk_section, aliases_section
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
        let mut idx = match crate::index::open(root).await {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("Failed to open index: {}", e);
                return 1;
            }
        };
        match idx.refresh().await {
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
        return run_setup_wizard(root);
    }

    0
}

/// Interactive rule setup wizard.
///
/// Delegates to `normalize_rules::setup::run_setup_wizard` which is also
/// available as `normalize rules setup`.
pub fn run_setup_wizard(root: &Path) -> i32 {
    normalize_rules::setup::run_setup_wizard(root)
}

/// Detect scratch directories in `root` that exist and are not already
/// gitignored. Returns the matching `SCRATCH_DIRS` entries.
///
/// "Already gitignored" is checked against the root `.gitignore` only — if a
/// scratch dir is covered there, the walker skips it for free, so listing it
/// in `[walk] exclude` would be redundant noise.
pub fn detect_scratch_dirs(root: &Path) -> Vec<(&'static str, &'static str)> {
    SCRATCH_DIRS
        .iter()
        .filter(|(dir, _)| {
            let path = root.join(dir.trim_end_matches('/'));
            path.exists() && !is_already_gitignored(root, dir)
        })
        .copied()
        .collect()
}

/// Returns true if `dir` (relative to `root`) is matched by the project's
/// `.gitignore`. Compiled via the same `ignore` crate the walker uses.
fn is_already_gitignored(root: &Path, dir: &str) -> bool {
    let gitignore_path = root.join(".gitignore");
    if !gitignore_path.exists() {
        return false;
    }
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    if builder.add(&gitignore_path).is_some() {
        return false;
    }
    let Ok(gi) = builder.build() else {
        return false;
    };
    let rel = std::path::Path::new(dir.trim_end_matches('/'));
    gi.matched_path_or_any_parents(rel, true).is_ignore()
}

/// Interactive prompt: for each detected scratch dir, ask the user whether to
/// add it to `[walk] exclude`. Default is "yes" (Enter accepts).
fn prompt_scratch_dirs(
    detected: &[(&'static str, &'static str)],
) -> Vec<&'static (&'static str, &'static str)> {
    println!("Detected scratch directories that aren't gitignored:");
    let stdin = io::stdin();
    let mut accepted = Vec::new();
    // Re-borrow against the original SCRATCH_DIRS table so we hand back
    // 'static references (callers iterate without lifetime juggling).
    for (dir, desc) in detected {
        print!("  Exclude {} ({})? [Y/n] ", dir, desc);
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let answer = line.trim().to_lowercase();
        if answer.is_empty() || answer == "y" || answer == "yes" {
            // Find matching static entry in SCRATCH_DIRS.
            if let Some(entry) = SCRATCH_DIRS.iter().find(|(d, _)| d == dir) {
                accepted.push(entry);
            }
        }
    }
    accepted
}

/// Detect task-tracking files (TODO.md, TASKS.md, etc.) in the project root.
pub fn detect_todo_files(root: &Path) -> Vec<String> {
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
pub fn update_gitignore(path: &Path) -> Vec<String> {
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

/// Preview what .gitignore changes would be made without writing.
pub fn preview_gitignore_changes(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let mut changes = Vec::new();

    for entry in GITIGNORE_ENTRIES {
        if matches!(find_entry(&lines, entry), EntryStatus::Missing) {
            changes.push(format!("Would add '{}' to .gitignore", entry));
        }
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

    #[tokio::test]
    async fn test_init_creates_moss_dir() {
        let tmp = tempdir().unwrap();
        let result = run_init(tmp.path(), false, false).await;
        assert_eq!(result, 0);
        assert!(tmp.path().join(".normalize").exists());
        assert!(tmp.path().join(".normalize/config.toml").exists());
    }

    #[tokio::test]
    async fn test_init_idempotent() {
        let tmp = tempdir().unwrap();
        let result1 = run_init(tmp.path(), false, false).await;
        let result2 = run_init(tmp.path(), false, false).await;
        assert_eq!(result1, 0);
        assert_eq!(result2, 0);
    }

    #[tokio::test]
    async fn test_init_updates_gitignore() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();

        run_init(tmp.path(), false, false).await;

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".normalize/*"));
        assert!(content.contains("!.normalize/config.toml"));
    }

    #[tokio::test]
    async fn test_init_skips_commented_entries() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "# .normalize\n").unwrap();

        run_init(tmp.path(), false, false).await;

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

    #[tokio::test]
    async fn test_init_inserts_near_existing() {
        let tmp = tempdir().unwrap();
        // Existing .gitignore already has .normalize/*
        fs::write(
            tmp.path().join(".gitignore"),
            "node_modules\n.normalize/*\nother_stuff\n",
        )
        .unwrap();

        run_init(tmp.path(), false, false).await;

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

    #[tokio::test]
    async fn test_init_detects_todo_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("TODO.md"), "# TODO\n").unwrap();
        fs::write(tmp.path().join("TASKS.md"), "# Tasks\n").unwrap();

        run_init(tmp.path(), false, false).await;

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        assert!(config.contains("[aliases]"));
        assert!(config.contains("TODO.md"));
        assert!(config.contains("TASKS.md"));
    }

    #[test]
    fn test_detect_scratch_dirs_absent() {
        let tmp = tempdir().unwrap();
        // Empty project — no .claude/worktrees → nothing detected.
        let detected = detect_scratch_dirs(tmp.path());
        assert!(detected.is_empty());
    }

    #[test]
    fn test_detect_scratch_dirs_present_and_not_gitignored() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".claude/worktrees")).unwrap();
        let detected = detect_scratch_dirs(tmp.path());
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0, ".claude/worktrees/");
    }

    #[test]
    fn test_detect_scratch_dirs_already_gitignored() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".claude/worktrees")).unwrap();
        fs::write(tmp.path().join(".gitignore"), ".claude/worktrees/\n").unwrap();
        let detected = detect_scratch_dirs(tmp.path());
        // Covered by .gitignore → walker skips for free → don't list in exclude.
        assert!(detected.is_empty());
    }

    #[tokio::test]
    async fn test_init_non_interactive_adds_detected_scratch_dirs() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".claude/worktrees")).unwrap();

        // Non-interactive (no TTY in test) → all detected scratch dirs added.
        run_init(tmp.path(), false, false).await;

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        assert!(
            config.contains("[walk]"),
            "missing [walk] section: {config}"
        );
        assert!(
            config.contains(".claude/worktrees/"),
            ".claude/worktrees/ should be excluded by default: {config}"
        );
        // Bootstrap opinion preserved.
        assert!(config.contains(".git/"));
    }

    #[test]
    fn test_init_writes_walk_exclude_with_git_when_no_scratch_dirs() {
        let tmp = tempdir().unwrap();
        // No .claude/worktrees → only bootstrap opinion in exclude.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(run_init(tmp.path(), false, false));
        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        assert!(config.contains("[walk]"));
        assert!(config.contains(".git/"));
        assert!(!config.contains(".claude/worktrees"));
    }

    #[tokio::test]
    async fn test_init_no_todo_files() {
        let tmp = tempdir().unwrap();

        run_init(tmp.path(), false, false).await;

        let config = fs::read_to_string(tmp.path().join(".normalize/config.toml")).unwrap();
        // Should not have aliases section if no todo-tracking files found
        assert!(!config.contains("[aliases]"));
    }
}
