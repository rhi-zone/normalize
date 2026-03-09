//! History management service for server-less CLI.

use server_less::cli;

/// History management sub-service.
pub struct HistoryService;

use crate::commands::history::{
    HistoryDiffReport, HistoryListReport, HistoryPruneReport, HistoryStatusReport,
    HistoryTreeReport,
};

impl std::fmt::Display for HistoryListReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::output::OutputFormatter;
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for HistoryDiffReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::output::OutputFormatter;
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for HistoryStatusReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::output::OutputFormatter;
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for HistoryTreeReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::output::OutputFormatter;
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for HistoryPruneReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::output::OutputFormatter;
        write!(f, "{}", self.format_text())
    }
}

#[cli(name = "history", description = "View shadow git edit history")]
impl HistoryService {
    /// List recent edit history
    pub fn list(
        &self,
        #[param(positional, help = "Filter history to specific file")] file: Option<String>,
        #[param(short = 'n', help = "Maximum number of entries to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryListReport, String> {
        use crate::shadow::Shadow;
        use std::path::PathBuf;

        let limit = limit.unwrap_or(20);
        let root = root
            .map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let shadow = Shadow::new(&root);
        if !shadow.exists() {
            return Ok(HistoryListReport::empty());
        }
        let entries = shadow.history(file.as_deref(), limit);
        let checkpoint = shadow.checkpoint();
        let head = entries.first().map(|e| e.id);
        Ok(HistoryListReport::new(head, checkpoint, entries))
    }

    /// Show diff for a specific commit
    pub fn diff(
        &self,
        #[param(positional, help = "Commit reference to diff")] commit_ref: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryDiffReport, String> {
        use crate::shadow::Shadow;
        use std::path::PathBuf;

        let root = root
            .map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let shadow = Shadow::new(&root);
        match shadow.diff(&commit_ref) {
            Some(diff) => Ok(HistoryDiffReport::new(commit_ref, diff)),
            None => Err(format!("Could not find commit: {}", commit_ref)),
        }
    }

    /// Show uncommitted shadow edits since last git commit
    pub fn status(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryStatusReport, String> {
        use crate::shadow::Shadow;
        use std::path::PathBuf;

        let root = root
            .map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let shadow = Shadow::new(&root);
        let entries = shadow.history(None, 100);
        let checkpoint = shadow.checkpoint();
        let count = entries
            .iter()
            .take_while(|e| {
                checkpoint
                    .as_ref()
                    .map(|c| &e.git_head != c)
                    .unwrap_or(true)
            })
            .count();
        Ok(HistoryStatusReport::new(count, checkpoint))
    }

    /// Show full tree structure of all branches
    pub fn tree(
        &self,
        #[param(short = 'n', help = "Maximum number of entries to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryTreeReport, String> {
        use crate::shadow::Shadow;
        use std::path::PathBuf;

        let limit = limit.unwrap_or(20);
        let root = root
            .map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let shadow = Shadow::new(&root);
        match shadow.tree(limit) {
            Some(tree_output) => {
                let lines: Vec<String> = tree_output.lines().map(|l| l.to_string()).collect();
                Ok(HistoryTreeReport::new(lines))
            }
            None => Err("Could not get tree view".to_string()),
        }
    }

    /// Prune shadow history, keeping only the last N commits
    pub fn prune(
        &self,
        #[param(positional, help = "Number of commits to keep")] keep: usize,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryPruneReport, String> {
        use crate::shadow::Shadow;
        use std::path::PathBuf;

        let root = root
            .map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let shadow = Shadow::new(&root);
        match shadow.prune(keep) {
            Ok(pruned_count) => Ok(HistoryPruneReport::new(pruned_count, keep)),
            Err(e) => Err(format!("Prune failed: {}", e)),
        }
    }
}
