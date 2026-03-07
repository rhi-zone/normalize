//! History command - view shadow git edit history.

use crate::output::OutputFormatter;
use crate::shadow::HistoryEntry;
use serde::Serialize;

/// History listing report (default mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryListReport {
    head: Option<usize>,
    checkpoint: Option<String>,
    edits: Vec<HistoryEntry>,
}

impl HistoryListReport {
    pub fn new(head: Option<usize>, checkpoint: Option<String>, edits: Vec<HistoryEntry>) -> Self {
        Self {
            head,
            checkpoint,
            edits,
        }
    }

    pub fn empty() -> Self {
        Self {
            head: None,
            checkpoint: None,
            edits: vec![],
        }
    }
}

impl OutputFormatter for HistoryListReport {
    fn format_text(&self) -> String {
        if self.edits.is_empty() {
            return "No edits in history".to_string();
        }
        let mut lines = Vec::new();
        for entry in &self.edits {
            let msg_suffix = entry
                .message
                .as_ref()
                .map(|m| format!(" \"{}\"", m))
                .unwrap_or_default();
            let head_marker = if Some(entry.id) == self.head {
                " [HEAD]"
            } else {
                ""
            };
            lines.push(format!(
                "  {}.{} {}: {} in {}{}",
                entry.id,
                head_marker,
                entry.operation,
                entry.target,
                entry.files.join(", "),
                msg_suffix
            ));
        }
        lines.join("\n")
    }
}

/// Diff report (--diff mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryDiffReport {
    #[serde(rename = "ref")]
    commit_ref: String,
    diff: String,
}

impl HistoryDiffReport {
    pub fn new(commit_ref: String, diff: String) -> Self {
        Self { commit_ref, diff }
    }
}

impl OutputFormatter for HistoryDiffReport {
    fn format_text(&self) -> String {
        self.diff.clone()
    }
}

/// Status report (--status mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryStatusReport {
    edits_since_checkpoint: usize,
    checkpoint: Option<String>,
}

impl HistoryStatusReport {
    pub fn new(edits_since_checkpoint: usize, checkpoint: Option<String>) -> Self {
        Self {
            edits_since_checkpoint,
            checkpoint,
        }
    }
}

impl OutputFormatter for HistoryStatusReport {
    fn format_text(&self) -> String {
        let mut lines = vec![format!(
            "Shadow edits since last commit: {}",
            self.edits_since_checkpoint
        )];
        if let Some(ref cp) = self.checkpoint {
            lines.push(format!("Last checkpoint: {}", cp));
        }
        lines.join("\n")
    }
}

/// Tree report (--all mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryTreeReport {
    tree: Vec<String>,
}

impl HistoryTreeReport {
    pub fn new(tree: Vec<String>) -> Self {
        Self { tree }
    }
}

impl OutputFormatter for HistoryTreeReport {
    fn format_text(&self) -> String {
        self.tree.join("\n")
    }
}

/// Prune report (--prune mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryPruneReport {
    pruned: usize,
    kept: usize,
}

impl HistoryPruneReport {
    pub fn new(pruned: usize, kept: usize) -> Self {
        Self { pruned, kept }
    }
}

impl OutputFormatter for HistoryPruneReport {
    fn format_text(&self) -> String {
        if self.pruned > 0 {
            format!(
                "Pruned {} commit{}, keeping last {}",
                self.pruned,
                if self.pruned == 1 { "" } else { "s" },
                self.kept
            )
        } else {
            format!("Nothing to prune (only {} commits in history)", self.kept)
        }
    }
}
