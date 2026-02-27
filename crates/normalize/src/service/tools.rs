//! Tools service for server-less CLI (lint + test).

use crate::commands::tools::lint::{LintListResult, LintRunResult};
use crate::commands::tools::test::{TestListResult, TestRunResult};
use crate::output::OutputFormatter;
use server_less::cli;

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

#[cli(name = "tools", about = "Run linters, formatters, and test runners")]
impl ToolsService {
    /// Run linters, formatters, and type checkers
    pub fn lint(&self) -> &LintService {
        &self.lint
    }

    /// Run native test runners
    pub fn test(&self) -> &TestService {
        &self.test
    }
}

#[cli(name = "lint", about = "Run linters, formatters, and type checkers")]
impl LintService {
    /// Run linters on the codebase
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
    ) -> Result<LintRunResult, String> {
        crate::commands::tools::lint::build_lint_run(
            target.as_deref(),
            root.as_deref().map(std::path::Path::new),
            fix,
            tools.as_deref(),
            category.as_deref(),
        )
    }

    /// List available linting tools
    pub fn list(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> LintListResult {
        crate::commands::tools::lint::build_lint_list(root.as_deref().map(std::path::Path::new))
    }
}

#[cli(name = "test", about = "Run native test runners")]
impl TestService {
    /// Run tests with auto-detected or specified runner
    pub fn run(
        &self,
        #[param(help = "Specific test runner (cargo, go, bun, npm, pytest)")] runner: Option<
            String,
        >,
        #[param(help = "Additional arguments to pass to the test runner")] args: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<TestRunResult, String> {
        crate::commands::tools::test::build_test_run(
            root.as_deref().map(std::path::Path::new),
            runner.as_deref(),
            &args,
        )
    }

    /// List available test runners
    pub fn list(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> TestListResult {
        crate::commands::tools::test::build_test_list(root.as_deref().map(std::path::Path::new))
    }
}
