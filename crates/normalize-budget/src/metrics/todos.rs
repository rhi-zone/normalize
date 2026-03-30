//! TODO/FIXME comment diff metric.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops;
use std::path::Path;

/// TODO/FIXME comments added or removed.
///
/// Returns measurements with `(file_path, todos_added, todos_removed)` by counting
/// lines containing `TODO` or `FIXME` in each changed file, comparing `base_ref` to HEAD.
pub struct TodoDeltaMetric;

impl DiffMetric for TodoDeltaMetric {
    fn name(&self) -> &'static str {
        "todos"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let repo = git_ops::open_repo(root)?;
        let changes = git_ops::diff_base_to_head(root, base_ref)?;

        let mut results = Vec::new();

        for change in changes {
            let old_count = change
                .old_id
                .and_then(|id| git_ops::read_blob_text(&repo, id))
                .map(|c| count_todos(&c))
                .unwrap_or(0);

            let new_count = change
                .new_id
                .and_then(|id| git_ops::read_blob_text(&repo, id))
                .map(|c| count_todos(&c))
                .unwrap_or(0);

            let added = new_count.saturating_sub(old_count) as f64;
            let removed = old_count.saturating_sub(new_count) as f64;

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

/// Count lines containing TODO or FIXME in a file's content.
fn count_todos(content: &str) -> usize {
    content
        .lines()
        .filter(|line| line.contains("TODO") || line.contains("FIXME"))
        .count()
}
