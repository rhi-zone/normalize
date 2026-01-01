//! Shadow Git - automatic edit history tracking.
//!
//! Maintains a hidden git repository (`.moss/shadow/`) that automatically
//! commits after each `moss edit` operation, preserving full edit history.

use crate::merge::Merge;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Shadow git configuration.
#[derive(Debug, Clone, Deserialize, Default, Merge)]
#[serde(default)]
pub struct ShadowConfig {
    /// Whether shadow git is enabled. Default: true
    pub enabled: Option<bool>,
    /// Confirm before deleting symbols. Default: true
    pub warn_on_delete: Option<bool>,
}

impl ShadowConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn warn_on_delete(&self) -> bool {
        self.warn_on_delete.unwrap_or(true)
    }
}

/// Information about an edit operation for shadow commit.
pub struct EditInfo {
    pub operation: String,
    pub target: String,
    pub files: Vec<PathBuf>,
    pub message: Option<String>,
    pub workflow: Option<String>,
}

/// Shadow git repository manager.
pub struct Shadow {
    /// Root of the project (where .moss/ lives)
    root: PathBuf,
    /// Path to shadow git directory (.moss/shadow/)
    shadow_dir: PathBuf,
    /// Path to shadow worktree (.moss/shadow/worktree/)
    worktree: PathBuf,
}

impl Shadow {
    /// Create a new Shadow instance for a project root.
    pub fn new(root: &Path) -> Self {
        let shadow_dir = root.join(".moss").join("shadow");
        let worktree = shadow_dir.join("worktree");
        Self {
            root: root.to_path_buf(),
            shadow_dir,
            worktree,
        }
    }

    /// Check if shadow git exists for this project.
    pub fn exists(&self) -> bool {
        self.shadow_dir.join(".git").exists()
    }

    /// Initialize shadow git repository if it doesn't exist.
    /// Called on first edit, not on `moss init`.
    fn init(&self) -> Result<(), ShadowError> {
        if self.exists() {
            return Ok(());
        }

        // Create worktree directory (git init will create .git inside shadow_dir)
        std::fs::create_dir_all(&self.worktree)
            .map_err(|e| ShadowError::Init(format!("Failed to create shadow directory: {}", e)))?;

        // Initialize git repo with worktree in subdirectory
        // Use --separate-git-dir to put .git in shadow_dir while worktree is in worktree/
        let status = Command::new("git")
            .args([
                "init",
                "--quiet",
                &format!(
                    "--separate-git-dir={}",
                    self.shadow_dir.join(".git").display()
                ),
            ])
            .current_dir(&self.worktree)
            .status()
            .map_err(|e| ShadowError::Init(format!("Failed to run git init: {}", e)))?;

        if !status.success() {
            return Err(ShadowError::Init("git init failed".to_string()));
        }

        // Configure git user for commits (shadow-specific, doesn't affect user's git)
        let _ = Command::new("git")
            .args(["config", "user.email", "shadow@moss.local"])
            .current_dir(&self.worktree)
            .status();
        let _ = Command::new("git")
            .args(["config", "user.name", "Moss Shadow"])
            .current_dir(&self.worktree)
            .status();

        Ok(())
    }

    /// Get the current git HEAD of the real repository.
    fn get_real_git_head(&self) -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&self.root)
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Copy a file to the shadow worktree, preserving relative path.
    fn copy_to_worktree(&self, file: &Path) -> Result<PathBuf, ShadowError> {
        let rel_path = file
            .strip_prefix(&self.root)
            .map_err(|_| ShadowError::Commit("File not under project root".to_string()))?;

        let dest = self.worktree.join(rel_path);

        // Create parent directories
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ShadowError::Commit(format!("Failed to create directories: {}", e)))?;
        }

        // Copy file
        std::fs::copy(file, &dest)
            .map_err(|e| ShadowError::Commit(format!("Failed to copy file: {}", e)))?;

        Ok(rel_path.to_path_buf())
    }

    /// Record file state before an edit.
    /// Call this before applying the edit to capture "before" state.
    pub fn before_edit(&self, files: &[&Path]) -> Result<(), ShadowError> {
        self.init()?;

        for file in files {
            if file.exists() {
                self.copy_to_worktree(file)?;
            }
        }

        Ok(())
    }

    /// Record file state after an edit and commit.
    /// Call this after applying the edit to capture "after" state.
    pub fn after_edit(&self, info: &EditInfo) -> Result<(), ShadowError> {
        // Copy updated files to worktree
        for file in &info.files {
            if file.exists() {
                self.copy_to_worktree(file)?;
            }
        }

        // Stage all changes (run in worktree directory)
        let status = Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.worktree)
            .status()
            .map_err(|e| ShadowError::Commit(format!("Failed to stage changes: {}", e)))?;

        if !status.success() {
            return Err(ShadowError::Commit("git add failed".to_string()));
        }

        // Check if there are changes to commit
        let status = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&self.worktree)
            .status()
            .map_err(|e| ShadowError::Commit(format!("Failed to check diff: {}", e)))?;

        if status.success() {
            // No changes to commit
            return Ok(());
        }

        // Build commit message
        let git_head = self
            .get_real_git_head()
            .unwrap_or_else(|| "none".to_string());
        let files_str: Vec<String> = info
            .files
            .iter()
            .filter_map(|f| f.strip_prefix(&self.root).ok())
            .map(|p| p.display().to_string())
            .collect();

        let mut commit_msg = format!("moss edit: {} {}\n\n", info.operation, info.target);

        if let Some(ref msg) = info.message {
            commit_msg.push_str(&format!("Message: {}\n", msg));
        }
        if let Some(ref wf) = info.workflow {
            commit_msg.push_str(&format!("Workflow: {}\n", wf));
        }
        commit_msg.push_str(&format!("Operation: {}\n", info.operation));
        commit_msg.push_str(&format!("Target: {}\n", info.target));
        commit_msg.push_str(&format!("Files: {}\n", files_str.join(", ")));
        commit_msg.push_str(&format!("Git-HEAD: {}\n", git_head));

        // Commit
        let status = Command::new("git")
            .args(["commit", "-m", &commit_msg])
            .current_dir(&self.worktree)
            .status()
            .map_err(|e| ShadowError::Commit(format!("Failed to commit: {}", e)))?;

        if !status.success() {
            return Err(ShadowError::Commit("git commit failed".to_string()));
        }

        Ok(())
    }

    /// Get the number of shadow commits (edits tracked).
    pub fn edit_count(&self) -> usize {
        if !self.exists() {
            return 0;
        }

        let output = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(&self.worktree)
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0),
            _ => 0,
        }
    }
}

/// Shadow git errors.
#[derive(Debug)]
pub enum ShadowError {
    Init(String),
    Commit(String),
}

impl std::fmt::Display for ShadowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShadowError::Init(msg) => write!(f, "Shadow init error: {}", msg),
            ShadowError::Commit(msg) => write!(f, "Shadow commit error: {}", msg),
        }
    }
}

impl std::error::Error for ShadowError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_shadow_new() {
        let dir = TempDir::new().unwrap();
        let shadow = Shadow::new(dir.path());

        assert!(!shadow.exists());
        assert_eq!(shadow.shadow_dir, dir.path().join(".moss").join("shadow"));
    }

    #[test]
    fn test_shadow_init() {
        let dir = TempDir::new().unwrap();
        let shadow = Shadow::new(dir.path());

        // Initialize as if it's the first edit
        shadow.init().unwrap();

        assert!(shadow.exists());
        assert!(shadow.worktree.exists());
    }

    #[test]
    fn test_shadow_before_after_edit() {
        let dir = TempDir::new().unwrap();

        // Create a test file
        let test_file = dir.path().join("test.rs");
        std::fs::write(&test_file, "fn foo() {}").unwrap();

        let shadow = Shadow::new(dir.path());

        // Before edit
        shadow.before_edit(&[&test_file]).unwrap();

        // Simulate edit
        std::fs::write(&test_file, "fn bar() {}").unwrap();

        // After edit
        let info = EditInfo {
            operation: "replace".to_string(),
            target: "test.rs/foo".to_string(),
            files: vec![test_file.clone()],
            message: Some("Renamed foo to bar".to_string()),
            workflow: None,
        };
        shadow.after_edit(&info).unwrap();

        assert_eq!(shadow.edit_count(), 1);
    }
}
