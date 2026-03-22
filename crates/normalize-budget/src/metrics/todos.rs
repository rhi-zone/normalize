//! TODO/FIXME comment diff metric.

use super::DiffMetric;
use std::path::Path;
use std::process::Command;

/// TODO/FIXME comments added or removed.
///
/// Returns `(file_path, todos_added, todos_removed)` triples by counting
/// lines matching `TODO` or `FIXME` in the diff output.
pub struct TodosMetric;

impl DiffMetric for TodosMetric {
    fn name(&self) -> &'static str {
        "todos"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<(String, f64, f64)>> {
        let output = Command::new("git")
            .args(["diff", base_ref, "--"])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git diff: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git diff failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut file_added: std::collections::HashMap<String, f64> = Default::default();
        let mut file_removed: std::collections::HashMap<String, f64> = Default::default();
        let mut current_file = String::new();

        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("+++ b/") {
                current_file = rest.to_string();
            } else if line.starts_with("--- ") || line.starts_with("diff --git") {
                // skip
            } else if line.starts_with('+') && !line.starts_with("+++") {
                let content = &line[1..];
                if content.contains("TODO") || content.contains("FIXME") {
                    *file_added.entry(current_file.clone()).or_default() += 1.0;
                }
            } else if line.starts_with('-') && !line.starts_with("---") {
                let content = &line[1..];
                if content.contains("TODO") || content.contains("FIXME") {
                    *file_removed.entry(current_file.clone()).or_default() += 1.0;
                }
            }
        }

        let mut all_files: std::collections::HashSet<String> = Default::default();
        all_files.extend(file_added.keys().cloned());
        all_files.extend(file_removed.keys().cloned());

        let results = all_files
            .into_iter()
            .filter(|f| !f.is_empty())
            .map(|f| {
                let added = file_added.get(&f).copied().unwrap_or(0.0);
                let removed = file_removed.get(&f).copied().unwrap_or(0.0);
                (f, added, removed)
            })
            .collect();

        Ok(results)
    }
}
