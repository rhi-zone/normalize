//! Module/file added or removed metric.

use super::{DiffMeasurement, DiffMetric};
use std::path::Path;
use std::process::Command;

/// Modules (files) added or removed.
///
/// Uses `git diff --name-status` to find files with status A (added) or D (deleted).
/// Returns a measurement with `added=1.0` for added files and `removed=1.0` for removed.
pub struct ModuleDeltaMetric;

impl DiffMetric for ModuleDeltaMetric {
    fn name(&self) -> &'static str {
        "modules"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
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
            let key = parts[1].trim().to_string();
            match status {
                "A" => results.push(DiffMeasurement {
                    key,
                    added: 1.0,
                    removed: 0.0,
                }),
                "D" => results.push(DiffMeasurement {
                    key,
                    added: 0.0,
                    removed: 1.0,
                }),
                _ => {}
            }
        }

        Ok(results)
    }
}
