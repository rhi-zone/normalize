//! Native rule integration for the ratchet system.
//!
//! Delegates to `normalize_ratchet::service::build_ratchet_diagnostics`.

use normalize_output::diagnostics::DiagnosticsReport;
use std::path::Path;

/// Build a DiagnosticsReport from the ratchet baseline check.
///
/// Called by the native rules engine. Returns an empty report if no baseline
/// exists or the check succeeds.
pub fn build_ratchet_report(root: &Path) -> DiagnosticsReport {
    let factory: normalize_ratchet::MetricFactory = normalize_ratchet::default_metrics;
    normalize_ratchet::service::build_ratchet_diagnostics(root, &factory)
}
