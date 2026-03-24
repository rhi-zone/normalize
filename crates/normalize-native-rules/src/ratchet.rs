//! Native rule integration for the ratchet system.
//!
//! Delegates to `normalize_ratchet::service::build_ratchet_diagnostics`.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use serde::Serialize;
use std::path::Path;

/// Report returned by the ratchet native rule check.
///
/// Wraps the `DiagnosticsReport` produced by the ratchet service so that it
/// can be formatted standalone (as text) or converted to a `DiagnosticsReport`
/// for the rules engine.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RatchetDiagnosticsReport(pub DiagnosticsReport);

impl OutputFormatter for RatchetDiagnosticsReport {
    fn format_text(&self) -> String {
        self.0.format_text()
    }

    fn format_pretty(&self) -> String {
        self.0.format_pretty()
    }
}

impl From<RatchetDiagnosticsReport> for DiagnosticsReport {
    fn from(report: RatchetDiagnosticsReport) -> Self {
        report.0
    }
}

/// Build a RatchetDiagnosticsReport from the ratchet baseline check.
///
/// Called by the native rules engine. Returns an empty report if no baseline
/// exists or the check succeeds.
pub fn build_ratchet_report(root: &Path) -> RatchetDiagnosticsReport {
    let factory: normalize_ratchet::MetricFactory = normalize_ratchet::default_metrics;
    RatchetDiagnosticsReport(normalize_ratchet::service::build_ratchet_diagnostics(
        root, &factory,
    ))
}
