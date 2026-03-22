//! Diff metric implementations for the budget system.

pub mod classes;
pub mod complexity_delta;
pub mod dependencies;
pub mod functions;
pub mod lines;
pub mod modules;
pub mod todos;

use std::path::Path;

/// A diff metric measures how much something has changed between a base ref and the working tree.
///
/// Returns `(key, added, removed)` triples for all items in the diff.
/// The framework filters by path prefix, aggregates added and removed separately,
/// then checks configured limits.
pub trait DiffMetric: Send + Sync {
    /// Short name used in budget entries (e.g. "lines", "functions").
    fn name(&self) -> &'static str;

    /// Returns `(key, added, removed)` triples for all changed items.
    ///
    /// - `key` is the address of the item (file path, symbol path, etc.)
    /// - `added` is the amount introduced
    /// - `removed` is the amount deleted
    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<(String, f64, f64)>>;
}
