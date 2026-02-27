//! Test command - run native test runners.

use std::path::Path;

use normalize_tools::test_runners::{all_runners, detect_test_runner, get_runner};

/// Run tests with auto-detected or specified runner.
pub fn cmd_test_run(root: Option<&Path>, runner: Option<&str>, args: &[String]) -> i32 {
    let root = root.unwrap_or_else(|| Path::new("."));

    let test_runner = if let Some(name) = runner {
        // Find specific runner by name
        get_runner(name)
    } else {
        // Auto-detect
        detect_test_runner(root)
    };

    let Some(test_runner) = test_runner else {
        eprintln!("No test runner detected for this project.");
        eprintln!("Supported: cargo (Rust), go (Go), bun/npm (JS/TS), pytest (Python)");
        return 1;
    };

    let info = test_runner.info();
    eprintln!("Running tests with {}...", info.name);

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    match test_runner.run(root, &args_refs) {
        Ok(result) => {
            if result.success() {
                0
            } else {
                result.status.code().unwrap_or(1)
            }
        }
        Err(e) => {
            eprintln!("Failed to run tests: {}", e);
            1
        }
    }
}

/// Test runner info for structured output.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TestRunnerItem {
    pub name: String,
    pub description: String,
    pub available: bool,
    pub detected: bool,
}

/// Test list result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TestListResult {
    pub runners: Vec<TestRunnerItem>,
}

impl std::fmt::Display for TestListResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Available test runners:\n")?;
        for runner in &self.runners {
            let status = if !runner.available {
                "(not installed)"
            } else if runner.detected {
                "(detected)"
            } else {
                ""
            };
            writeln!(
                f,
                "  {:10} - {} {}",
                runner.name, runner.description, status
            )?;
        }
        Ok(())
    }
}

/// Test run result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TestRunResult {
    pub runner: String,
    pub success: bool,
    pub exit_code: i32,
}

impl std::fmt::Display for TestRunResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.success {
            write!(f, "Tests passed ({})", self.runner)
        } else {
            write!(
                f,
                "Tests failed ({}, exit code {})",
                self.runner, self.exit_code
            )
        }
    }
}

/// Build test list (data only).
pub fn build_test_list(root: Option<&Path>) -> TestListResult {
    let root = root.unwrap_or_else(|| Path::new("."));
    let runners: Vec<TestRunnerItem> = all_runners()
        .iter()
        .map(|runner| {
            let info = runner.info();
            TestRunnerItem {
                name: info.name.to_string(),
                description: info.description.to_string(),
                available: runner.is_available(),
                detected: runner.detect(root) > 0.0,
            }
        })
        .collect();
    TestListResult { runners }
}

/// Run tests and return structured result (data only).
pub fn build_test_run(
    root: Option<&Path>,
    runner: Option<&str>,
    args: &[String],
) -> Result<TestRunResult, String> {
    let root = root.unwrap_or_else(|| Path::new("."));

    let test_runner = if let Some(name) = runner {
        get_runner(name)
    } else {
        detect_test_runner(root)
    };

    let Some(test_runner) = test_runner else {
        return Err("No test runner detected for this project".to_string());
    };

    let info = test_runner.info();
    let runner_name = info.name.to_string();
    eprintln!("Running tests with {}...", runner_name);

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    match test_runner.run(root, &args_refs) {
        Ok(result) => {
            let exit_code = result.status.code().unwrap_or(1);
            Ok(TestRunResult {
                runner: runner_name,
                success: result.success(),
                exit_code: if result.success() { 0 } else { exit_code },
            })
        }
        Err(e) => Err(format!("Failed to run tests: {}", e)),
    }
}

/// List available test runners.
pub fn cmd_test_list(root: Option<&Path>) -> i32 {
    let root = root.unwrap_or_else(|| Path::new("."));

    println!("Available test runners:\n");

    for runner in all_runners() {
        let info = runner.info();
        let available = runner.is_available();
        let score = runner.detect(root);

        let status = if !available {
            "(not installed)"
        } else if score > 0.0 {
            "(detected)"
        } else {
            ""
        };

        println!("  {:10} - {} {}", info.name, info.description, status);
    }

    0
}
