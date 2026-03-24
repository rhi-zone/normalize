//! Native rule integration for the budget system.
//!
//! Delegates to `normalize_budget::service::build_budget_report`.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use serde::Serialize;
use std::path::Path;

/// Report returned by the budget native rule check.
///
/// Wraps the `DiagnosticsReport` produced by the budget service so that it
/// can be formatted standalone (as text) or converted to a `DiagnosticsReport`
/// for the rules engine.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BudgetDiagnosticsReport(pub DiagnosticsReport);

impl OutputFormatter for BudgetDiagnosticsReport {
    fn format_text(&self) -> String {
        self.0.format_text()
    }

    fn format_pretty(&self) -> String {
        self.0.format_pretty()
    }
}

impl From<BudgetDiagnosticsReport> for DiagnosticsReport {
    fn from(report: BudgetDiagnosticsReport) -> Self {
        report.0
    }
}

/// Build a BudgetDiagnosticsReport from the budget check for use in `normalize rules run`.
///
/// Called by the native rules engine. Returns an empty report if no budget file
/// exists or all limits are within bounds.
pub fn build_budget_report(root: &Path) -> BudgetDiagnosticsReport {
    let factory: normalize_budget::DiffMetricFactory = normalize_budget::default_diff_metrics;
    BudgetDiagnosticsReport(normalize_budget::service::build_budget_report(
        root, &factory,
    ))
}
