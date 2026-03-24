//! Classes/structs/types added/removed diff metric.

use super::functions::symbol_diff;
use super::{DiffMeasurement, DiffMetric};
use std::path::Path;

/// Classes, structs, and types introduced or removed.
///
/// Returns a measurement with `added=1.0` for added and `removed=1.0` for removed.
pub struct ClassDeltaMetric;

impl DiffMetric for ClassDeltaMetric {
    fn name(&self) -> &'static str {
        "classes"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        symbol_diff(
            root,
            base_ref,
            &["class", "struct", "type", "interface", "enum"],
        )
    }
}
