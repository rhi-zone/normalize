//! Tools service for server-less CLI (lint + test).

use crate::commands::tools::lint::{LintListResult, LintRunResult};
use crate::commands::tools::test::{TestListResult, TestRunResult};
use crate::output::OutputFormatter;
use server_less::cli;
use std::path::PathBuf;

/// Tools management service (lint + test subcommands).
pub struct ToolsService {
    lint: LintService,
    test: TestService,
}

impl Default for ToolsService {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolsService {
    pub fn new() -> Self {
        Self {
            lint: LintService,
            test: TestService,
        }
    }
}

/// Lint sub-service.
pub struct LintService;

/// Test sub-service.
pub struct TestService;

impl std::fmt::Display for LintListResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

fn discover_repos(dir: &str, depth: usize) -> Result<Vec<PathBuf>, String> {
    crate::multi_repo::discover_repos_depth(&PathBuf::from(dir), depth)
}

#[cli(
    name = "tools",
    description = "Run linters, formatters, and test runners"
)]
impl ToolsService {
    /// Run linters, formatters, and type checkers
    ///
    /// Examples:
    ///   normalize tools lint run             # run all detected linters
    ///   normalize tools lint list            # list available linting tools
    pub fn lint(&self) -> &LintService {
        &self.lint
    }

    /// Run native test runners
    ///
    /// Examples:
    ///   normalize tools test run             # run auto-detected test runner
    ///   normalize tools test list            # list available test runners
    pub fn test(&self) -> &TestService {
        &self.test
    }
}

#[cli(
    name = "lint",
    description = "Run linters, formatters, and type checkers"
)]
impl LintService {
    /// Run linters on the codebase
    ///
    /// Examples:
    ///   normalize tools lint run                          # run all detected linters
    ///   normalize tools lint run src/                     # lint a specific path
    ///   normalize tools lint run -f                       # auto-fix issues
    ///   normalize tools lint run -t clippy,eslint         # run specific tools only
    ///   normalize tools lint run -c fmt                   # run only formatters
    ///   normalize tools lint run --repos-dir ~/projects   # lint across multiple repos
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        #[param(positional, help = "Target path to check")] target: Option<String>,
        #[param(short = 'f', help = "Fix issues automatically")] fix: bool,
        #[param(short = 't', help = "Specific tools to run (comma-separated)")] tools: Option<
            String,
        >,
        #[param(short = 'c', help = "Filter by category: lint, fmt, type")] category: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Run across all git repos under DIR")] repos_dir: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<LintRunResult, String> {
        if let Some(dir) = repos_dir {
            let repo_paths = discover_repos(&dir, repos_depth.unwrap_or(1))?;
            crate::commands::tools::lint::build_lint_run_multi(
                &repo_paths,
                fix,
                tools.as_deref(),
                category.as_deref(),
            )
        } else {
            crate::commands::tools::lint::build_lint_run(
                target.as_deref(),
                root.as_deref().map(std::path::Path::new),
                fix,
                tools.as_deref(),
                category.as_deref(),
            )
        }
    }

    /// List available linting tools
    ///
    /// Examples:
    ///   normalize tools lint list            # show detected linters for this project
    pub fn list(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> LintListResult {
        crate::commands::tools::lint::build_lint_list(root.as_deref().map(std::path::Path::new))
    }
}

#[cli(name = "test", description = "Run native test runners")]
impl TestService {
    /// Run tests with auto-detected or specified runner
    ///
    /// Examples:
    ///   normalize tools test run                          # run with auto-detected runner
    ///   normalize tools test run --runner cargo            # use cargo test
    ///   normalize tools test run --repos-dir ~/projects   # run tests across multiple repos
    pub fn run(
        &self,
        #[param(help = "Specific test runner (cargo, go, bun, npm, pytest)")] runner: Option<
            String,
        >,
        #[param(help = "Additional arguments to pass to the test runner")] args: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Run across all git repos under DIR")] repos_dir: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<TestRunResult, String> {
        if let Some(dir) = repos_dir {
            let repo_paths = discover_repos(&dir, repos_depth.unwrap_or(1))?;
            crate::commands::tools::test::build_test_run_multi(
                &repo_paths,
                runner.as_deref(),
                &args,
            )
        } else {
            crate::commands::tools::test::build_test_run(
                root.as_deref().map(std::path::Path::new),
                runner.as_deref(),
                &args,
            )
        }
    }

    /// List available test runners
    ///
    /// Examples:
    ///   normalize tools test list            # show detected test runners for this project
    pub fn list(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> TestListResult {
        crate::commands::tools::test::build_test_list(root.as_deref().map(std::path::Path::new))
    }
}
