//! Native rule integration for the budget system.
//!
//! Delegates to `normalize_budget::service::build_budget_diagnostics`.

use normalize_output::diagnostics::DiagnosticsReport;
use std::path::Path;

/// Build a DiagnosticsReport from the budget check for use in `normalize rules run`.
///
/// Called by the native rules engine. Returns an empty report if no budget file
/// exists or all limits are within bounds.
pub fn build_budget_report(root: &Path) -> DiagnosticsReport {
    let factory: normalize_budget::DiffMetricFactory = normalize_budget::default_diff_metrics;
    normalize_budget::service::build_budget_diagnostics(root, &factory)
}
