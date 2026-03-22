//! Module/file added or removed metric.

use super::DiffMetric;
use std::path::Path;
use std::process::Command;

/// Modules (files) added or removed.
///
/// Uses `git diff --name-status` to find files with status A (added) or D (deleted).
/// Returns `(file_path, 1.0, 0.0)` for added files and `(file_path, 0.0, 1.0)` for removed.
pub struct ModulesMetric;

impl DiffMetric for ModulesMetric {
    fn name(&self) -> &'static str {
        "modules"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<(String, f64, f64)>> {
        let output = Command::new("git")
            .args(["diff", "--name-status", base_ref, "--"])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git diff: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git diff --name-status failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() != 2 {
                continue;
            }
            let status = parts[0].trim();
            let file = parts[1].trim().to_string();
            match status {
                "A" => results.push((file, 1.0, 0.0)),
                "D" => results.push((file, 0.0, 1.0)),
                _ => {}
            }
        }

        Ok(results)
    }
}
