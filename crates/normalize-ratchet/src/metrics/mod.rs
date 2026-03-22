//! Metric implementations for the ratchet system.

pub mod call_complexity;
pub mod complexity;
pub mod file_stats;

use std::path::Path;

/// A measurable metric for the ratchet system.
///
/// Implementations return `(address, value)` pairs for all items in the repo.
/// The framework filters by path prefix and aggregates.
///
/// Symbol address format:
/// - Function-level: `{file}/{Parent}/{fn}` or `{file}/{fn}` (no parent)
/// - File-level: `{file}`
/// - Directory-level: `{dir}/`
pub trait Metric: Send + Sync {
    /// Short name used in baseline entries (e.g. "complexity").
    fn name(&self) -> &'static str;

    /// Return `(address, value)` pairs for ALL measurable items in the repo.
    /// The framework handles filtering and aggregation.
    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>>;

    /// True if higher values are worse (e.g. complexity).
    /// False if lower values are worse (e.g. coverage).
    fn higher_is_worse(&self) -> bool {
        true
    }
}
