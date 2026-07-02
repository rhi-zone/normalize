//! Initialize normalize in a project directory.

use std::fs;
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

/// The generated `[walk]` section plus a human-readable summary of what it
/// seeded. Returned by [`build_walk_section`].
pub struct WalkSection {
    /// The rendered `[walk]` TOML block to splice into `config.toml`.
    pub toml: String,
    /// One-line summary of the seeded excludes, for the `InitReport` change log
    /// / dry-run preview.
    pub summary: String,
}

/// Build the `[walk]` section for a freshly-created `config.toml`.
///
/// Always seeds the daemon baseline (`.git/`, `.normalize/`) so the section is
/// discoverable and self-documenting, then appends any auto-detected scratch
/// dirs (e.g. `.claude/worktrees/`) present under `root` and not already
/// gitignored.
///
/// This is called from the live `init` command in `service::mod`, mirroring how
/// that command already reuses `detect_todo_files` / `update_gitignore` from
/// this module. The scratch-dir path table (`SCRATCH_DIRS`) lives here in the
/// integration layer per `CLAUDE.md`, never in the rule-engine crates.
pub fn build_walk_section(root: &Path) -> WalkSection {
    let mut excludes: Vec<String> = vec![".git/".to_string(), ".normalize/".to_string()];
    for (dir, _) in detect_scratch_dirs(root) {
        excludes.push(dir.to_string());
    }
    let exclude_lines = excludes
        .iter()
        .map(|s| format!("    \"{}\",", s))
        .collect::<Vec<_>>()
        .join("\n");
    let toml = format!(
        r#"
[walk]
# Project-wide path exclusions (gitignore-style; affects walker AND index).
exclude = [
{}
]
"#,
        exclude_lines
    );
    let summary = format!("Seeded [walk] exclude: {}", excludes.join(", "));
    WalkSection { toml, summary }
}

/// Interactive rule setup wizard.
///
/// Delegates to `normalize_rules::setup::run_setup_wizard` which is also
/// available as `normalize rules setup`.
pub fn run_setup_wizard(root: &Path) -> i32 {
    normalize_rules::setup::run_setup_wizard(root, false)
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

// These exercise the library helpers reused by the live `init` command in
// `service::mod` (`build_walk_section`, `detect_scratch_dirs`,
// `update_gitignore`, `detect_todo_files`, `preview_gitignore_changes`). The
// command itself resolves its root from `env::current_dir`, so it is covered by
// an integration test in the scratch dir rather than here — testing the helpers
// directly keeps coverage without cwd juggling.
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_update_gitignore_adds_entries() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();

        update_gitignore(&tmp.path().join(".gitignore"));

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".normalize/*"));
        assert!(content.contains("!.normalize/config.toml"));
    }

    #[test]
    fn test_update_gitignore_skips_commented_entries() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "# .normalize\n").unwrap();

        update_gitignore(&tmp.path().join(".gitignore"));

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // .normalize should remain commented (not added as active entry)
        assert!(lines.iter().any(|l| l.trim() == "# .normalize"));
        assert!(!lines.iter().any(|l| l.trim() == ".normalize"));

        // But negation entries should still be added
        assert!(lines.iter().any(|l| l.trim() == "!.normalize/config.toml"));
    }

    #[test]
    fn test_update_gitignore_inserts_near_existing() {
        let tmp = tempdir().unwrap();
        // Existing .gitignore already has .normalize/*
        fs::write(
            tmp.path().join(".gitignore"),
            "node_modules\n.normalize/*\nother_stuff\n",
        )
        .unwrap();

        update_gitignore(&tmp.path().join(".gitignore"));

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
    fn test_preview_gitignore_changes_reports_missing() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
        let changes = preview_gitignore_changes(&tmp.path().join(".gitignore"));
        assert!(changes.iter().any(|c| c.contains(".normalize/*")));
        // Preview must not mutate the file.
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(!content.contains(".normalize/*"));
    }

    #[test]
    fn test_detect_todo_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("TODO.md"), "# TODO\n").unwrap();
        fs::write(tmp.path().join("TASKS.md"), "# Tasks\n").unwrap();

        let files = detect_todo_files(tmp.path());
        assert!(files.contains(&"TODO.md".to_string()));
        assert!(files.contains(&"TASKS.md".to_string()));
    }

    #[test]
    fn test_detect_todo_files_none() {
        let tmp = tempdir().unwrap();
        assert!(detect_todo_files(tmp.path()).is_empty());
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

    #[test]
    fn test_build_walk_section_baseline_when_no_scratch_dirs() {
        let tmp = tempdir().unwrap();
        // No .claude/worktrees → only the daemon baseline in exclude.
        let walk = build_walk_section(tmp.path());
        assert!(walk.toml.contains("[walk]"));
        assert!(walk.toml.contains(".git/"));
        assert!(walk.toml.contains(".normalize/"));
        assert!(!walk.toml.contains(".claude/worktrees"));
        assert!(walk.summary.contains(".git/"));
        assert!(walk.summary.contains(".normalize/"));
    }

    #[test]
    fn test_build_walk_section_adds_detected_scratch_dirs() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".claude/worktrees")).unwrap();
        let walk = build_walk_section(tmp.path());
        assert!(
            walk.toml.contains("[walk]"),
            "missing [walk] section: {}",
            walk.toml
        );
        assert!(
            walk.toml.contains(".claude/worktrees/"),
            ".claude/worktrees/ should be excluded by default: {}",
            walk.toml
        );
        // Baseline preserved alongside the detected scratch dir.
        assert!(walk.toml.contains(".git/"));
        assert!(walk.toml.contains(".normalize/"));
        assert!(walk.summary.contains(".claude/worktrees/"));
    }
}
