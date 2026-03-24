//! Line-level diff metric using `git diff --numstat`.

use super::{DiffMeasurement, DiffMetric};
use std::path::Path;
use std::process::Command;

/// Line-level diff per file.
///
/// Returns measurements with `(file_path, lines_added, lines_removed)` for every changed file.
pub struct LineDeltaMetric;

impl DiffMetric for LineDeltaMetric {
    fn name(&self) -> &'static str {
        "lines"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let output = Command::new("git")
            .args(["diff", "--numstat", base_ref, "--"])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git diff: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git diff --numstat failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            // Format: "<added>\t<removed>\t<file>"
            // Binary files show "-\t-\t<file>" — skip them.
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() != 3 {
                continue;
            }
            let added: f64 = match parts[0].parse() {
                Ok(v) => v,
                Err(_) => continue, // binary file indicator "-"
            };
            let removed: f64 = match parts[1].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let key = parts[2].trim().to_string();
            results.push(DiffMeasurement {
                key,
                added,
                removed,
            });
        }

        Ok(results)
    }
}
