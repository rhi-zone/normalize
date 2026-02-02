//! pytest runner for Python projects.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{TestResult, TestRunner, TestRunnerInfo};

pub struct Pytest;

impl Pytest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Pytest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner for Pytest {
    fn info(&self) -> TestRunnerInfo {
        TestRunnerInfo {
            name: "pytest",
            description: "Python test runner (pytest)",
        }
    }

    fn is_available(&self) -> bool {
        // Try common locations
        for cmd in ["pytest", "python3 -m pytest", "python -m pytest"] {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let result = Command::new(parts[0])
                .args(&parts[1..])
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if result.map(|s| s.success()).unwrap_or(false) {
                return true;
            }
        }
        false
    }

    fn detect(&self, root: &Path) -> f32 {
        if root.join("pytest.ini").exists() {
            return 1.0;
        }
        if root.join("pyproject.toml").exists() {
            if let Ok(content) = std::fs::read_to_string(root.join("pyproject.toml"))
                && content.contains("[tool.pytest")
            {
                return 1.0;
            }
            return 0.8;
        }
        if root.join("setup.py").exists() || root.join("requirements.txt").exists() {
            return 0.6;
        }
        0.0
    }

    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult> {
        // Try pytest directly first, fall back to python -m pytest
        let status = Command::new("pytest")
            .args(args)
            .current_dir(root)
            .status()
            .or_else(|_| {
                Command::new("python3")
                    .arg("-m")
                    .arg("pytest")
                    .args(args)
                    .current_dir(root)
                    .status()
            })?;

        Ok(TestResult {
            runner: "pytest".into(),
            status,
        })
    }
}
