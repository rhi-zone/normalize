//! History command - view shadow git edit history.

use crate::output::OutputFormatter;
use crate::shadow::{HistoryEntry, Shadow};
use serde::Serialize;
use std::path::PathBuf;

/// History listing report (default mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HistoryListReport {
    head: Option<usize>,
    checkpoint: Option<String>,
    edits: Vec<HistoryEntry>,
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

// ── Service-callable functions ────────────────────────────────────────

/// Service-callable: list history entries.
pub fn cmd_list_service(
    root: Option<&str>,
    file: Option<&str>,
    limit: usize,
) -> Result<HistoryListReport, String> {
    let root = root
        .map(PathBuf::from)
        // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let shadow = Shadow::new(&root);
    if !shadow.exists() {
        return Ok(HistoryListReport {
            head: None,
            checkpoint: None,
            edits: vec![],
        });
    }
    let entries = shadow.history(file, limit);
    let checkpoint = shadow.checkpoint();
    let head = entries.first().map(|e| e.id);
    Ok(HistoryListReport {
        head,
        checkpoint,
        edits: entries,
    })
}

/// Service-callable: show diff for a commit.
pub fn cmd_diff_service(root: Option<&str>, commit_ref: &str) -> Result<HistoryDiffReport, String> {
    let root = root
        .map(PathBuf::from)
        // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let shadow = Shadow::new(&root);
    match shadow.diff(commit_ref) {
        Some(diff) => Ok(HistoryDiffReport {
            commit_ref: commit_ref.to_string(),
            diff,
        }),
        None => Err(format!("Could not find commit: {}", commit_ref)),
    }
}

/// Service-callable: show status of shadow edits.
pub fn cmd_status_service(root: Option<&str>) -> Result<HistoryStatusReport, String> {
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
    Ok(HistoryStatusReport {
        edits_since_checkpoint: count,
        checkpoint,
    })
}

/// Service-callable: show full tree structure.
pub fn cmd_tree_service(root: Option<&str>, limit: usize) -> Result<HistoryTreeReport, String> {
    let root = root
        .map(PathBuf::from)
        // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let shadow = Shadow::new(&root);
    match shadow.tree(limit) {
        Some(tree_output) => {
            let lines: Vec<String> = tree_output.lines().map(|l| l.to_string()).collect();
            Ok(HistoryTreeReport { tree: lines })
        }
        None => Err("Could not get tree view".to_string()),
    }
}

/// Service-callable: prune shadow history.
pub fn cmd_prune_service(root: Option<&str>, keep: usize) -> Result<HistoryPruneReport, String> {
    let root = root
        .map(PathBuf::from)
        // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let shadow = Shadow::new(&root);
    match shadow.prune(keep) {
        Ok(pruned_count) => Ok(HistoryPruneReport {
            pruned: pruned_count,
            kept: keep,
        }),
        Err(e) => Err(format!("Prune failed: {}", e)),
    }
}
