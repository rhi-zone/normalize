//! Line-level diff metric using gix.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops::{self, FileChangeKind};
use std::path::Path;

/// Line-level diff per file.
///
/// Returns measurements with `(file_path, lines_added, lines_removed)` for every changed file.
/// Compares the committed state at `base_ref` against HEAD (working-tree uncommitted changes
/// are not included; the budget is enforced at commit/CI time).
pub struct LineDeltaMetric;

impl DiffMetric for LineDeltaMetric {
    fn name(&self) -> &'static str {
        "lines"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let repo = git_ops::open_repo(root)?;
        let changes = git_ops::diff_base_to_head(root, base_ref)?;

        let mut results = Vec::new();

        for change in changes {
            let old_lines = change
                .old_id
                .and_then(|id| git_ops::read_blob_bytes(&repo, id))
                .map(|b| count_lines(&b))
                .unwrap_or(0);

            let new_lines = change
                .new_id
                .and_then(|id| git_ops::read_blob_bytes(&repo, id))
                .map(|b| count_lines(&b))
                .unwrap_or(0);

            let added = new_lines.saturating_sub(old_lines) as f64;
            let removed = old_lines.saturating_sub(new_lines) as f64;

            // Skip files where nothing changed in terms of line count
            if added == 0.0 && removed == 0.0 && change.kind == FileChangeKind::Modified {
                continue;
            }

            if added > 0.0 || removed > 0.0 {
                results.push(DiffMeasurement {
                    key: change.path,
                    added,
                    removed,
                });
            }
        }

        Ok(results)
    }
}

fn count_lines(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    data.iter().filter(|&&b| b == b'\n').count() + 1
}
