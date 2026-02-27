//! Directory context: hierarchical context files.
//!
//! Collects and merges `.context.md` and `CONTEXT.md` files from the directory
//! hierarchy, from project root to target path.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::output::OutputFormatter;

/// Context file names to look for (in priority order).
const CONTEXT_FILES: &[&str] = &[".context.md", "CONTEXT.md"];

/// Context file list report (--list mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContextListReport {
    paths: Vec<String>,
}

impl ContextListReport {
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }
}

impl OutputFormatter for ContextListReport {
    fn format_text(&self) -> String {
        if self.paths.is_empty() {
            "No context files found.".to_string()
        } else {
            self.paths.join("\n")
        }
    }
}

/// A single context file entry.
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ContextEntry {
    path: String,
    content: String,
}

/// Context report (default mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContextReport {
    entries: Vec<ContextEntry>,
}

impl ContextReport {
    /// Build from (relative_path, content) pairs.
    pub fn new(entries: Vec<(String, String)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(path, content)| ContextEntry { path, content })
                .collect(),
        }
    }
}

impl OutputFormatter for ContextReport {
    fn format_text(&self) -> String {
        if self.entries.is_empty() {
            return "No context files found.".to_string();
        }
        let mut lines = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            lines.push(format!("# {}", entry.path));
            lines.push(String::new());
            lines.push(entry.content.clone());
        }
        lines.join("\n")
    }
}

/// Collect context files from root to target directory.
/// Returns files in order from root to target (most general to most specific).
pub fn collect_context_files(root: &Path, target_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Build path from root to target
    let mut dirs = Vec::new();
    let mut current = target_dir.to_path_buf();

    // Walk up from target to root, collecting directories
    while current.starts_with(root) {
        dirs.push(current.clone());
        if current == root {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }

    // Reverse to get root-to-target order
    dirs.reverse();

    // Check each directory for context files
    for dir in dirs {
        for name in CONTEXT_FILES {
            let path = dir.join(name);
            if path.exists() {
                files.push(path);
                break; // Only take first match per directory
            }
        }
    }

    files
}

/// Get merged context content for a path.
/// Used by other commands (e.g., view --dir-context).
pub fn get_merged_context(root: &Path, target: &Path) -> Option<String> {
    // Find the target directory - walk up from target until we find an existing dir
    let target_dir = if target.is_file() {
        target.parent().unwrap_or(root).to_path_buf()
    } else if target.is_dir() {
        target.to_path_buf()
    } else {
        // Target doesn't exist - find first existing parent
        let mut dir = target.to_path_buf();
        while !dir.exists() {
            match dir.parent() {
                Some(p) => dir = p.to_path_buf(),
                None => return None,
            }
        }
        dir
    };

    let root = root.canonicalize().ok()?;
    let target_dir = target_dir.canonicalize().ok()?;

    let files = collect_context_files(&root, &target_dir);
    if files.is_empty() {
        return None;
    }

    let mut content = String::new();
    for (i, file) in files.iter().enumerate() {
        if i > 0 {
            content.push_str("\n\n");
        }
        if let Ok(text) = fs::read_to_string(file) {
            content.push_str(&text);
        }
    }

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_collect_single_context_file() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();

        let files = collect_context_files(root, root);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("CONTEXT.md"));
    }

    #[test]
    fn test_collect_hierarchical_context() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/.context.md"), "Src context").unwrap();

        let files = collect_context_files(root, &root.join("src"));
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("CONTEXT.md"));
        assert!(files[1].ends_with(".context.md"));
    }

    #[test]
    fn test_dotfile_takes_priority() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Uppercase").unwrap();
        fs::write(root.join(".context.md"), "Dotfile").unwrap();

        let files = collect_context_files(root, root);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".context.md"));
    }

    #[test]
    fn test_get_merged_context() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/.context.md"), "Sub").unwrap();

        let content = get_merged_context(root, &root.join("sub/file.rs")).unwrap();
        assert!(content.contains("Root"));
        assert!(content.contains("Sub"));
    }

    #[test]
    fn test_no_context_files() {
        let tmp = tempdir().unwrap();
        let files = collect_context_files(tmp.path(), tmp.path());
        assert!(files.is_empty());
    }
}
