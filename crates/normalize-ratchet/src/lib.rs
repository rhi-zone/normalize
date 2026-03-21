//! Metric regression-tracking ("ratchet") for normalize.
//!
//! Stores a baseline of metric values in `.normalize/ratchet.json` (checked into git),
//! and flags when current values regress past the baseline.
//!
//! # Architecture
//!
//! - [`Metric`] trait: implement to add a new tracked metric
//! - [`baseline`]: baseline file I/O (`.normalize/ratchet.json`)
//! - [`complexity`]: built-in cyclomatic complexity metric
//! - [`check`]: regression detection logic
//! - [`update`]: baseline update logic
//!
//! The `cli` feature adds a [`service::RatchetService`] with `check`, `update`, and `show`
//! subcommands, suitable for mounting in the main `normalize` binary.

pub mod baseline;
pub mod check;
pub mod complexity;
pub mod update;

#[cfg(feature = "cli")]
pub mod service;

use std::path::Path;

/// Factory function type for constructing the set of active metrics.
/// Lives outside the `cli` feature so the rules engine can reference it without CLI deps.
pub type MetricFactory = fn() -> Vec<Box<dyn Metric>>;

/// A measurable metric that can be tracked in the ratchet baseline.
pub trait Metric: Send + Sync {
    /// Short identifier for this metric (used as the key in `ratchet.json`).
    fn name(&self) -> &'static str;

    /// Measure the metric across `root`, returning `(key, value)` pairs.
    ///
    /// Aggregate metrics use a sentinel key (e.g. `"::total"`).
    /// Per-function metrics use the `file/Parent/function` or `file/function` addressing
    /// that matches `normalize view` output.
    fn measure(&self, root: &Path) -> anyhow::Result<Vec<(String, i64)>>;

    /// Returns `true` if `current` is worse than `baseline` (i.e. a regression).
    fn is_regression(&self, baseline: i64, current: i64) -> bool;
}
