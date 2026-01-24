//! Cargo test runner for Rust projects.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{TestResult, TestRunner, TestRunnerInfo};

pub struct CargoTest;

impl CargoTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner for CargoTest {
    fn info(&self) -> TestRunnerInfo {
        TestRunnerInfo {
            name: "cargo",
            description: "Rust test runner (cargo test)",
        }
    }

    fn is_available(&self) -> bool {
        Command::new("cargo")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn detect(&self, root: &Path) -> f32 {
        if root.join("Cargo.toml").exists() {
            1.0
        } else {
            0.0
        }
    }

    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult> {
        let mut cmd = Command::new("cargo");
        cmd.arg("test").args(args).current_dir(root);

        let status = cmd.status()?;
        Ok(TestResult {
            runner: "cargo".into(),
            status,
        })
    }
}
