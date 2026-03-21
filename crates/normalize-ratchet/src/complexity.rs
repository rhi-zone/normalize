//! Cyclomatic complexity metric implementation.
//!
//! Produces two kinds of entries per run:
//! - Per-function: key `file/Parent/function` or `file/function`, value = complexity score.
//! - Aggregate: key `::total`, value = sum of all function complexities.
//!
//! # Usage from the `normalize` binary
//!
//! The `normalize` binary constructs a [`ComplexityMetric`] (via
//! `normalize_ratchet::complexity::ComplexityMetric`) and passes it to
//! [`crate::check::check_regressions`] or [`crate::update::update_baseline`].
//! The actual analysis is performed by a user-supplied function that matches
//! the [`MeasureFn`] signature — this breaks the would-be circular dependency
//! between `normalize-ratchet` and `normalize`.

use std::path::Path;

use anyhow::Result;

use crate::Metric;

/// Sentinel key for the aggregate (sum) complexity value.
pub const TOTAL_KEY: &str = "::total";

/// Type alias for the measurement function passed at construction time.
pub type MeasureFn = fn(&Path) -> Result<Vec<(String, i64)>>;

/// Cyclomatic complexity metric.
///
/// The actual analysis is injected via `measure_fn` so that the `normalize-ratchet`
/// crate does not need to depend on the `normalize` binary crate (which would create
/// a circular dependency).
///
/// # Construction
///
/// ```rust,ignore
/// use normalize_ratchet::complexity::ComplexityMetric;
///
/// let metric = ComplexityMetric::new(my_measure_fn);
/// ```
pub struct ComplexityMetric {
    measure_fn: MeasureFn,
}

impl ComplexityMetric {
    /// Create a new `ComplexityMetric` backed by the given measurement function.
    pub fn new(measure_fn: MeasureFn) -> Self {
        Self { measure_fn }
    }
}

impl Metric for ComplexityMetric {
    fn name(&self) -> &'static str {
        "complexity"
    }

    fn measure(&self, root: &Path) -> Result<Vec<(String, i64)>> {
        (self.measure_fn)(root)
    }

    fn is_regression(&self, baseline: i64, current: i64) -> bool {
        current > baseline
    }
}

/// Build a ratchet key for a function.
///
/// Format: `file/Parent/name` or `file/name` (no parent).
/// Uses forward slashes regardless of OS — matches `normalize view` addressing.
pub fn function_key(file_path: &str, parent: Option<&str>, name: &str) -> String {
    // Normalise path separators
    let file = file_path.replace('\\', "/");
    match parent {
        Some(p) => format!("{file}/{p}/{name}"),
        None => format!("{file}/{name}"),
    }
}
