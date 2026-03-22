//! Shared metric primitives for the ratchet and budget systems.
//!
//! This crate provides:
//! - The [`Metric`] trait for snapshot metrics
//! - [`MetricFactory`] type alias
//! - [`Aggregate`] enum and [`aggregate`] function
//! - Path filtering utilities for `(key, value)` pairs

use std::path::Path;

mod aggregate;
mod filter;

pub use aggregate::{Aggregate, aggregate};
pub use filter::filter_by_prefix;

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

/// Factory function type: produce all metrics for a repo root.
/// Lives outside the `cli` feature so `normalize-native-rules` can use it.
pub type MetricFactory = fn(root: &Path) -> Vec<Box<dyn Metric>>;
