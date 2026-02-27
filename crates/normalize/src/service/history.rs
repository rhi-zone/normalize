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

#[cli(name = "history", about = "View shadow git edit history")]
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
        let limit = limit.unwrap_or(20);
        crate::commands::history::cmd_list_service(root.as_deref(), file.as_deref(), limit)
    }

    /// Show diff for a specific commit
    pub fn diff(
        &self,
        #[param(positional, help = "Commit reference to diff")] commit_ref: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryDiffReport, String> {
        crate::commands::history::cmd_diff_service(root.as_deref(), &commit_ref)
    }

    /// Show uncommitted shadow edits since last git commit
    pub fn status(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryStatusReport, String> {
        crate::commands::history::cmd_status_service(root.as_deref())
    }

    /// Show full tree structure of all branches
    pub fn tree(
        &self,
        #[param(short = 'n', help = "Maximum number of entries to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryTreeReport, String> {
        let limit = limit.unwrap_or(20);
        crate::commands::history::cmd_tree_service(root.as_deref(), limit)
    }

    /// Prune shadow history, keeping only the last N commits
    pub fn prune(
        &self,
        #[param(positional, help = "Number of commits to keep")] keep: usize,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<HistoryPruneReport, String> {
        crate::commands::history::cmd_prune_service(root.as_deref(), keep)
    }
}
