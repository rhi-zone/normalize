//! Go test runner.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{TestResult, TestRunner, TestRunnerInfo};

pub struct GoTest;

impl GoTest {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner for GoTest {
    fn info(&self) -> TestRunnerInfo {
        TestRunnerInfo {
            name: "go",
            description: "Go test runner (go test)",
        }
    }

    fn is_available(&self) -> bool {
        Command::new("go")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn detect(&self, root: &Path) -> f32 {
        if root.join("go.mod").exists() {
            1.0
        } else if root.join("go.sum").exists() {
            0.9
        } else {
            0.0
        }
    }

    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult> {
        let mut cmd = Command::new("go");
        cmd.arg("test");

        if args.is_empty() {
            cmd.arg("./...");
        } else {
            cmd.args(args);
        }

        cmd.current_dir(root);

        let status = cmd.status()?;
        Ok(TestResult {
            runner: "go".into(),
            status,
        })
    }
}
