//! Classes/structs/types added/removed diff metric.

use super::DiffMetric;
use super::functions::symbol_diff;
use std::path::Path;

/// Classes, structs, and types introduced or removed.
///
/// Returns `(file/Parent/name, 1.0, 0.0)` for added and `(file/Parent/name, 0.0, 1.0)` for removed.
pub struct ClassesMetric;

impl DiffMetric for ClassesMetric {
    fn name(&self) -> &'static str {
        "classes"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<(String, f64, f64)>> {
        symbol_diff(
            root,
            base_ref,
            &["class", "struct", "type", "interface", "enum"],
        )
    }
}
