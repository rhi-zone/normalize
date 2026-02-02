//! History command - view shadow git edit history.

use crate::shadow::Shadow;
use clap::Args;
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

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(HistoryArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run history command.
pub fn run(
    args: HistoryArgs,
    format: crate::output::OutputFormat,
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
    let root = args
        .root
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let shadow = Shadow::new(&root);

    if !shadow.exists() {
        if format.is_json() {
            println!(
                "{}",
                serde_json::json!({
                    "head": null,
                    "checkpoint": null,
                    "edits": []
                })
            );
        } else {
            println!("No shadow history (no edits tracked yet)");
        }
        return 0;
    }

    // Handle --diff
    if let Some(ref commit_ref) = args.diff {
        return cmd_diff(&shadow, commit_ref, format.is_json());
    }

    // Handle --status
    if args.status {
        return cmd_status(&shadow, format.is_json());
    }

    // Handle --all (tree view)
    if args.all {
        return cmd_tree(&shadow, args.limit, format.is_json());
    }

    // Handle --prune
    if let Some(keep) = args.prune {
        return cmd_prune(&shadow, keep, format.is_json());
    }

    // Regular history listing
    let entries = shadow.history(args.file.as_deref(), args.limit);

    if format.is_json() {
        let checkpoint = shadow.checkpoint();
        let head = entries.first().map(|e| e.id);

        // Build JSON output matching design spec
        let edits: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "operation": e.operation,
                    "target": e.target,
                    "files": e.files,
                    "message": e.message,
                    "workflow": e.workflow,
                    "git_head": e.git_head,
                    "timestamp": e.timestamp
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "head": head,
                "checkpoint": checkpoint,
                "edits": edits
            })
        );
    } else {
        if entries.is_empty() {
            println!("No edits in history");
            return 0;
        }

        for entry in &entries {
            let msg_suffix = entry
                .message
                .as_ref()
                .map(|m| format!(" \"{}\"", m))
                .unwrap_or_default();

            let head_marker = if entry.id == entries.first().map(|e| e.id).unwrap_or(0) {
                " [HEAD]"
            } else {
                ""
            };

            println!(
                "  {}.{} {}: {} in {}{}",
                entry.id,
                head_marker,
                entry.operation,
                entry.target,
                entry.files.join(", "),
                msg_suffix
            );
        }
    }

    0
}

/// Show diff for a specific commit.
fn cmd_diff(shadow: &Shadow, commit_ref: &str, json: bool) -> i32 {
    match shadow.diff(commit_ref) {
        Some(diff) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ref": commit_ref,
                        "diff": diff
                    })
                );
            } else {
                print!("{}", diff);
            }
            0
        }
        None => {
            eprintln!("Could not find commit: {}", commit_ref);
            1
        }
    }
}

/// Show status of shadow edits since last real git commit.
fn cmd_status(shadow: &Shadow, json: bool) -> i32 {
    let entries = shadow.history(None, 100);
    let checkpoint = shadow.checkpoint();

    // Count edits since checkpoint
    let edits_since_checkpoint: Vec<_> = entries
        .iter()
        .take_while(|e| {
            checkpoint
                .as_ref()
                .map(|c| &e.git_head != c)
                .unwrap_or(true)
        })
        .collect();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "edits_since_checkpoint": edits_since_checkpoint.len(),
                "checkpoint": checkpoint
            })
        );
    } else {
        println!(
            "Shadow edits since last commit: {}",
            edits_since_checkpoint.len()
        );
        if let Some(cp) = checkpoint {
            println!("Last checkpoint: {}", cp);
        }
    }

    0
}

/// Show full tree structure of shadow history (all branches).
fn cmd_tree(shadow: &Shadow, limit: usize, json: bool) -> i32 {
    match shadow.tree(limit) {
        Some(tree_output) => {
            if json {
                // Parse tree into structured format
                let lines: Vec<&str> = tree_output.lines().collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "tree": lines
                    })
                );
            } else {
                print!("{}", tree_output);
            }
            0
        }
        None => {
            eprintln!("Could not get tree view");
            1
        }
    }
}

/// Prune shadow history, keeping only the last N commits.
fn cmd_prune(shadow: &Shadow, keep: usize, json: bool) -> i32 {
    match shadow.prune(keep) {
        Ok(pruned_count) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "pruned": pruned_count,
                        "kept": keep
                    })
                );
            } else if pruned_count > 0 {
                println!(
                    "Pruned {} commit{}, keeping last {}",
                    pruned_count,
                    if pruned_count == 1 { "" } else { "s" },
                    keep
                );
            } else {
                println!("Nothing to prune (only {} commits in history)", keep);
            }
            0
        }
        Err(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": e.to_string()
                    })
                );
            } else {
                eprintln!("Prune failed: {}", e);
            }
            1
        }
    }
}
