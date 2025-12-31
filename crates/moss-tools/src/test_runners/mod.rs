//! Test runner adapters for different ecosystems.
//!
//! Each test runner detects whether it applies to a project and runs the native test command.

mod bun;
mod cargo;
mod go;
mod npm;
mod pytest;

pub use bun::BunTest;
pub use cargo::CargoTest;
pub use go::GoTest;
pub use npm::NpmTest;
pub use pytest::Pytest;

use std::path::Path;
use std::process::ExitStatus;

/// Information about a test runner.
#[derive(Debug, Clone)]
pub struct TestRunnerInfo {
    pub name: &'static str,
    pub description: &'static str,
}

/// Result of running tests.
#[derive(Debug)]
pub struct TestResult {
    pub runner: String,
    pub status: ExitStatus,
}

impl TestResult {
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

/// A test runner that can detect and run tests for a project type.
pub trait TestRunner: Send + Sync {
    /// Info about this test runner.
    fn info(&self) -> TestRunnerInfo;

    /// Check if this test runner is available (binary exists).
    fn is_available(&self) -> bool;

    /// Detect if this runner applies to the project. Returns confidence 0.0-1.0.
    fn detect(&self, root: &Path) -> f32;

    /// Run tests, streaming output to stdout/stderr.
    fn run(&self, root: &Path, args: &[&str]) -> std::io::Result<TestResult>;
}

/// Get all available test runners.
pub fn all_test_runners() -> Vec<Box<dyn TestRunner>> {
    vec![
        Box::new(CargoTest::new()),
        Box::new(GoTest::new()),
        Box::new(BunTest::new()),
        Box::new(NpmTest::new()),
        Box::new(Pytest::new()),
    ]
}

/// Find the best test runner for a project.
pub fn detect_test_runner(root: &Path) -> Option<Box<dyn TestRunner>> {
    let runners = all_test_runners();

    let mut best: Option<(f32, Box<dyn TestRunner>)> = None;

    for runner in runners {
        if !runner.is_available() {
            continue;
        }

        let score = runner.detect(root);
        if score > 0.0 {
            if best.is_none() || score > best.as_ref().unwrap().0 {
                best = Some((score, runner));
            }
        }
    }

    best.map(|(_, runner)| runner)
}
