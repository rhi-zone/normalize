//! Bun test runner for JavaScript/TypeScript projects.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{TestResult, TestRunner, TestRunnerInfo};

pub struct BunTest;

impl BunTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BunTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner for BunTest {
    fn info(&self) -> TestRunnerInfo {
        TestRunnerInfo {
            name: "bun",
            description: "Bun test runner (bun test)",
        }
    }

    fn is_available(&self) -> bool {
        Command::new("bun")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn detect(&self, root: &Path) -> f32 {
        // Prefer bun if bun.lockb exists
        if root.join("bun.lockb").exists() {
            1.0
        } else if root.join("package.json").exists() {
            // Could be a JS project but prefer npm/other runners
            0.5
        } else {
            0.0
        }
    }

    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult> {
        let mut cmd = Command::new("bun");
        cmd.arg("test").args(args).current_dir(root);

        let status = cmd.status()?;
        Ok(TestResult {
            runner: "bun".into(),
            status,
        })
    }
}
