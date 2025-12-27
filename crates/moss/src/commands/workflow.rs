//! Workflow command - Lua-based workflows.

use std::path::Path;

use clap::Subcommand;

#[cfg(feature = "lua")]
use crate::workflow::LuaRuntime;

#[derive(Subcommand)]
pub enum WorkflowAction {
    /// List available workflows
    List,

    /// Run a workflow
    Run {
        /// Workflow name or path to .lua file
        workflow: String,

        /// Task description (available as `task` variable in Lua)
        #[arg(short, long)]
        task: Option<String>,
    },
}

pub fn cmd_workflow(action: WorkflowAction, root: Option<&Path>, json: bool) -> i32 {
    match action {
        WorkflowAction::List => cmd_workflow_list(root, json),
        WorkflowAction::Run { workflow, task } => {
            cmd_workflow_run(&workflow, task.as_deref(), root, json)
        }
    }
}

fn cmd_workflow_list(root: Option<&Path>, json: bool) -> i32 {
    let root = root.unwrap_or_else(|| Path::new("."));
    let workflows_dir = root.join(".moss").join("workflows");

    if !workflows_dir.exists() {
        if json {
            println!("[]");
        } else {
            println!("No workflows directory at .moss/workflows/");
        }
        return 0;
    }

    let mut workflows = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "lua").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    workflows.push(name.to_string());
                }
            }
        }
    }

    workflows.sort();

    if json {
        println!("{}", serde_json::to_string(&workflows).unwrap());
    } else if workflows.is_empty() {
        println!("No workflows found in .moss/workflows/");
    } else {
        for name in workflows {
            println!("{}", name);
        }
    }

    0
}

#[cfg(feature = "lua")]
fn cmd_workflow_run(workflow: &str, task: Option<&str>, root: Option<&Path>, json: bool) -> i32 {
    let root = root.unwrap_or_else(|| Path::new("."));

    let workflow_path = if workflow.ends_with(".lua") {
        root.join(workflow)
    } else {
        root.join(".moss")
            .join("workflows")
            .join(format!("{}.lua", workflow))
    };

    if !workflow_path.exists() {
        eprintln!("Workflow not found: {}", workflow_path.display());
        return 1;
    }

    let runtime = match LuaRuntime::new(root) {
        Ok(r) => r,
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            } else {
                eprintln!("Failed to create Lua runtime: {}", e);
            }
            return 1;
        }
    };

    // Set task variable if provided
    if let Some(t) = task {
        if let Err(e) = runtime.run_string(&format!("task = {:?}", t)) {
            eprintln!("Failed to set task: {}", e);
            return 1;
        }
    }

    match runtime.run_file(&workflow_path) {
        Ok(()) => {
            if json {
                println!("{}", serde_json::json!({"success": true}));
            }
            0
        }
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({"error": e.to_string()}));
            } else {
                eprintln!("Lua error: {}", e);
            }
            1
        }
    }
}

#[cfg(not(feature = "lua"))]
fn cmd_workflow_run(
    _workflow: &str,
    _task: Option<&str>,
    _root: Option<&Path>,
    _json: bool,
) -> i32 {
    eprintln!("Lua workflows require the 'lua' feature");
    eprintln!("Rebuild with: cargo build --features lua");
    1
}
