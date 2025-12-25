//! TOML-based workflow engine.
//!
//! Workflows orchestrate moss primitives (view, edit, analyze) through:
//! - Step-based execution (linear sequence)
//! - State machine execution (conditional transitions)
//!
//! LLM is an optional plugin, not required for workflow execution.
//! Enable the "llm" feature to use LLM-powered workflows.

mod config;
mod execute;
mod llm;
mod strategies;

pub use config::{load_workflow, WorkflowConfig};
pub use execute::run_workflow;
pub use llm::{build_llm_strategy, LlmStrategy};
