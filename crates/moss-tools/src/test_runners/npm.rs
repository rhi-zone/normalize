//! npm test runner for JavaScript/TypeScript projects.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{TestResult, TestRunner, TestRunnerInfo};

pub struct NpmTest;

impl NpmTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NpmTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner for NpmTest {
    fn info(&self) -> TestRunnerInfo {
        TestRunnerInfo {
            name: "npm",
            description: "npm test runner (npm test)",
        }
    }

    fn is_available(&self) -> bool {
        Command::new("npm")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn detect(&self, root: &Path) -> f32 {
        if root.join("package-lock.json").exists() {
            1.0
        } else if root.join("package.json").exists() {
            0.7
        } else {
            0.0
        }
    }

    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult> {
        let mut cmd = Command::new("npm");
        cmd.arg("test");

        if !args.is_empty() {
            cmd.arg("--").args(args);
        }

        cmd.current_dir(root);

        let status = cmd.status()?;
        Ok(TestResult {
            runner: "npm".into(),
            status,
        })
    }
}
