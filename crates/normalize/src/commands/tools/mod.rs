//! External ecosystem tools (linters, formatters, test runners)

use clap::Subcommand;
use std::path::Path;

use crate::output::OutputFormat;

pub mod lint;
pub mod test;

#[derive(Subcommand)]
pub enum ToolsAction {
    /// Run linters, formatters, and type checkers
    Lint {
        #[command(subcommand)]
        action: Option<LintSubAction>,

        /// Target path to check (defaults to current directory)
        #[arg(global = true)]
        target: Option<String>,

        /// Fix issues automatically where possible
        #[arg(short, long, global = true)]
        fix: bool,

        /// Specific tools to run (comma-separated, e.g., "ruff,oxlint")
        #[arg(short, long, global = true)]
        tools: Option<String>,

        /// Filter by category: lint, fmt, type
        #[arg(short, long, global = true)]
        category: Option<String>,

        /// Output in SARIF format
        #[arg(long, global = true)]
        sarif: bool,

        /// Watch for file changes and re-run on save
        #[arg(short, long, global = true)]
        watch: bool,
    },

    /// Run native test runners (cargo test, go test, bun test, etc.)
    Test {
        #[command(subcommand)]
        action: Option<TestSubAction>,

        /// Specific test runner to use (cargo, go, bun, npm, pytest)
        #[arg(short = 'R', long, global = true)]
        runner: Option<String>,

        /// Additional arguments to pass to the test runner
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum LintSubAction {
    /// Run linters (default)
    Run,
    /// List available linting tools
    List,
}

#[derive(Subcommand)]
pub enum TestSubAction {
    /// Run tests (default)
    Run,
    /// List available test runners
    List,
}

/// Print JSON schema for the tools subcommand's output type.
fn print_tools_schema(action: &ToolsAction) -> i32 {
    match action {
        ToolsAction::Lint { action: sub, .. } => {
            if matches!(sub, Some(LintSubAction::List)) {
                crate::output::print_output_schema::<lint::LintListResult>();
                0
            } else {
                eprintln!("Lint run subcommand does not have a structured output schema");
                1
            }
        }
        ToolsAction::Test { .. } => {
            eprintln!("Test subcommand does not have a structured output schema");
            1
        }
    }
}

pub fn run(
    action: ToolsAction,
    root: Option<&Path>,
    format: OutputFormat,
    json: bool,
    output_schema: bool,
) -> i32 {
    if output_schema {
        return print_tools_schema(&action);
    }
    match action {
        ToolsAction::Lint {
            action: sub_action,
            target,
            fix,
            tools,
            category,
            sarif,
            watch,
        } => {
            let is_list = matches!(sub_action, Some(LintSubAction::List));
            if is_list {
                lint::cmd_lint_list(root, &format)
            } else if watch {
                lint::cmd_lint_watch(
                    target.as_deref(),
                    root,
                    fix,
                    tools.as_deref(),
                    category.as_deref(),
                    json,
                )
            } else {
                lint::cmd_lint_run(
                    target.as_deref(),
                    root,
                    fix,
                    tools.as_deref(),
                    category.as_deref(),
                    sarif,
                    format,
                )
            }
        }
        ToolsAction::Test {
            action: sub_action,
            runner,
            args,
        } => {
            let is_list = matches!(sub_action, Some(TestSubAction::List));
            if is_list {
                test::cmd_test_list(root)
            } else {
                test::cmd_test_run(root, runner.as_deref(), &args)
            }
        }
    }
}
