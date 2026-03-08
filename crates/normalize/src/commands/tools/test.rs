//! Test command - run native test runners.

use crate::output::OutputFormatter;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use normalize_tools::test_runners::{all_runners, detect_test_runner, get_runner};

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

/// Per-repo test result for multi-repo mode.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RepoTestResult {
    pub name: String,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub runner: String,
    pub success: bool,
    pub exit_code: i32,
}

/// Test run result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TestRunResult {
    pub runner: String,
    pub success: bool,
    pub exit_code: i32,
    /// Populated in multi-repo mode (--repos-dir).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<RepoTestResult>>,
}

impl OutputFormatter for TestRunResult {
    fn format_text(&self) -> String {
        if let Some(repos) = &self.repos {
            if repos.is_empty() {
                return "No repositories found".to_string();
            }
            let mut out = String::new();
            for repo in repos {
                use std::fmt::Write as _;
                writeln!(out, "=== {} ===", repo.name).unwrap();
                if let Some(err) = &repo.error {
                    writeln!(out, "Error: {}", err).unwrap();
                } else if repo.success {
                    writeln!(out, "Tests passed ({})", repo.runner).unwrap();
                } else {
                    writeln!(
                        out,
                        "Tests failed ({}, exit code {})",
                        repo.runner, repo.exit_code
                    )
                    .unwrap();
                }
                writeln!(out).unwrap();
            }
            out
        } else if self.success {
            format!("Tests passed ({})", self.runner)
        } else {
            format!(
                "Tests failed ({}, exit code {})",
                self.runner, self.exit_code
            )
        }
    }
}

impl std::fmt::Display for TestRunResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
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

/// Run tests across multiple repos and return aggregated results.
pub fn build_test_run_multi(
    repos: &[PathBuf],
    runner: Option<&str>,
    args: &[String],
) -> Result<TestRunResult, String> {
    let entries: Vec<RepoTestResult> = repos
        .par_iter()
        .map(|repo_path| {
            let name = repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            match build_test_run(Some(repo_path), runner, args) {
                Ok(r) => RepoTestResult {
                    name,
                    path: repo_path.clone(),
                    error: None,
                    runner: r.runner,
                    success: r.success,
                    exit_code: r.exit_code,
                },
                Err(e) => RepoTestResult {
                    name,
                    path: repo_path.clone(),
                    error: Some(e),
                    runner: String::new(),
                    success: false,
                    exit_code: 1,
                },
            }
        })
        .collect();

    let all_passed = entries.iter().all(|r| r.error.is_none() && r.success);
    let runner_name = entries
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.runner.as_str())
        .next()
        .unwrap_or("multi")
        .to_string();

    Ok(TestRunResult {
        runner: runner_name,
        success: all_passed,
        exit_code: if all_passed { 0 } else { 1 },
        repos: Some(entries),
    })
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
                repos: None,
            })
        }
        Err(e) => Err(format!("Failed to run tests: {}", e)),
    }
}
