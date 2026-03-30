//! Module/file added or removed metric.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops::{self, FileChangeKind};
use std::path::Path;

/// Modules (files) added or removed.
///
/// Uses gix to diff the tree at `base_ref` against HEAD, reporting files with
/// status Added or Deleted. Returns a measurement with `added=1.0` for added
/// files and `removed=1.0` for removed files.
pub struct ModuleDeltaMetric;

impl DiffMetric for ModuleDeltaMetric {
    fn name(&self) -> &'static str {
        "modules"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let changes = git_ops::diff_base_to_head(root, base_ref)?;

        let mut results = Vec::new();

        for change in changes {
            match change.kind {
                FileChangeKind::Added => results.push(DiffMeasurement {
                    key: change.path,
                    added: 1.0,
                    removed: 0.0,
                }),
                FileChangeKind::Deleted => results.push(DiffMeasurement {
                    key: change.path,
                    added: 0.0,
                    removed: 1.0,
                }),
                FileChangeKind::Modified => {} // not a module addition/removal
            }
        }

        Ok(results)
    }
}
