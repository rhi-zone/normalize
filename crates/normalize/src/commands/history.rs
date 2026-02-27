//! History command - view shadow git edit history.

use crate::output::OutputFormatter;
use crate::shadow::{HistoryEntry, Shadow};
use clap::Args;
use serde::Serialize;
use std::path::PathBuf;

/// History command arguments.
#[derive(Args, Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HistoryArgs {
    /// Filter history to specific file
    pub file: Option<String>,

    /// Root directory (defaults to current directory)
    #[arg(short, long)]
    pub root: Option<PathBuf>,

    /// Show full tree structure (all branches)
    #[arg(long)]
    #[serde(default)]
    pub all: bool,

    /// Show uncommitted shadow edits since last git commit
    #[arg(long)]
    #[serde(default)]
    pub status: bool,

    /// Show diff for a specific commit
    #[arg(long, value_name = "REF")]
    pub diff: Option<String>,

    /// Maximum number of entries to show
    #[arg(short = 'n', long, default_value = "20")]
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Prune shadow history, keeping only the last N commits
    #[arg(long, value_name = "KEEP")]
    pub prune: Option<usize>,
}

/// Helper for serde default limit
fn default_limit() -> usize {
    20
}

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

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(HistoryArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Determine which output schema to print based on the args mode.
fn print_history_output_schema(args: &HistoryArgs) {
    if args.diff.is_some() {
        crate::output::print_output_schema::<HistoryDiffReport>();
    } else if args.status {
        crate::output::print_output_schema::<HistoryStatusReport>();
    } else if args.all {
        crate::output::print_output_schema::<HistoryTreeReport>();
    } else if args.prune.is_some() {
        crate::output::print_output_schema::<HistoryPruneReport>();
    } else {
        crate::output::print_output_schema::<HistoryListReport>();
    }
}

/// Run history command.
pub fn run(
    args: HistoryArgs,
    format: crate::output::OutputFormat,
    output_schema: bool,
    input_schema: bool,
    params_json: Option<&str>,
) -> i32 {
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
    if output_schema {
        print_history_output_schema(&args);
        return 0;
    }
    let root = args
        .root
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let shadow = Shadow::new(&root);

    if !shadow.exists() {
        let report = HistoryListReport {
            head: None,
            checkpoint: None,
            edits: vec![],
        };
        report.print(&format);
        return 0;
    }

    // Handle --diff
    if let Some(ref commit_ref) = args.diff {
        return cmd_diff(&shadow, commit_ref, &format);
    }

    // Handle --status
    if args.status {
        return cmd_status(&shadow, &format);
    }

    // Handle --all (tree view)
    if args.all {
        return cmd_tree(&shadow, args.limit, &format);
    }

    // Handle --prune
    if let Some(keep) = args.prune {
        return cmd_prune(&shadow, keep, &format);
    }

    // Regular history listing
    let entries = shadow.history(args.file.as_deref(), args.limit);
    let checkpoint = shadow.checkpoint();
    let head = entries.first().map(|e| e.id);

    let report = HistoryListReport {
        head,
        checkpoint,
        edits: entries,
    };
    report.print(&format);

    0
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

/// Show diff for a specific commit.
fn cmd_diff(shadow: &Shadow, commit_ref: &str, format: &crate::output::OutputFormat) -> i32 {
    match shadow.diff(commit_ref) {
        Some(diff) => {
            let report = HistoryDiffReport {
                commit_ref: commit_ref.to_string(),
                diff,
            };
            report.print(format);
            0
        }
        None => {
            eprintln!("Could not find commit: {}", commit_ref);
            1
        }
    }
}

/// Show status of shadow edits since last real git commit.
fn cmd_status(shadow: &Shadow, format: &crate::output::OutputFormat) -> i32 {
    let entries = shadow.history(None, 100);
    let checkpoint = shadow.checkpoint();

    // Count edits since checkpoint
    let count = entries
        .iter()
        .take_while(|e| {
            checkpoint
                .as_ref()
                .map(|c| &e.git_head != c)
                .unwrap_or(true)
        })
        .count();

    let report = HistoryStatusReport {
        edits_since_checkpoint: count,
        checkpoint,
    };
    report.print(format);

    0
}

/// Show full tree structure of shadow history (all branches).
fn cmd_tree(shadow: &Shadow, limit: usize, format: &crate::output::OutputFormat) -> i32 {
    match shadow.tree(limit) {
        Some(tree_output) => {
            let lines: Vec<String> = tree_output.lines().map(|l| l.to_string()).collect();
            let report = HistoryTreeReport { tree: lines };
            report.print(format);
            0
        }
        None => {
            eprintln!("Could not get tree view");
            1
        }
    }
}

/// Prune shadow history, keeping only the last N commits.
fn cmd_prune(shadow: &Shadow, keep: usize, format: &crate::output::OutputFormat) -> i32 {
    match shadow.prune(keep) {
        Ok(pruned_count) => {
            let report = HistoryPruneReport {
                pruned: pruned_count,
                kept: keep,
            };
            report.print(format);
            0
        }
        Err(e) => {
            eprintln!("Prune failed: {}", e);
            1
        }
    }
}
