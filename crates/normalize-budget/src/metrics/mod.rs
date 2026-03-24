//! Diff metric implementations for the budget system.

pub mod classes;
pub mod complexity_delta;
pub mod dependencies;
pub mod functions;
pub mod lines;
pub mod modules;
pub mod todos;

use std::path::Path;

/// A single measurement from a diff metric.
#[derive(Debug, Clone)]
pub struct DiffMeasurement {
    /// Address of the item (file path, symbol path, etc.).
    pub key: String,
    /// Amount of the item introduced (added lines, new symbols, etc.).
    pub added: f64,
    /// Amount of the item deleted (removed lines, dropped symbols, etc.).
    pub removed: f64,
}

/// A diff metric measures how much something has changed between a base ref and the working tree.
///
/// Returns [`DiffMeasurement`] values for all items in the diff.
/// The framework filters by path prefix, aggregates added and removed separately,
/// then checks configured limits.
pub trait DiffMetric: Send + Sync {
    /// Short name used in budget entries (e.g. "lines", "functions").
    fn name(&self) -> &'static str;

    /// Returns measurements for all changed items.
    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>>;
}
