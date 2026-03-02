//! Unified coverage command — groups test-ratio, test-gaps, and budget views.

use crate::analyze::test_gaps::TestGapsReport;
use crate::commands::analyze::budget::BudgetReport;
use crate::commands::analyze::test_ratio::TestRatioReport;
use normalize_output::OutputFormatter;
use serde::Serialize;

/// Coverage analysis output — test ratio, test gaps, or line budget breakdown.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "view")]
pub enum CoverageOutput {
    /// Test/impl line ratio per module
    #[serde(rename = "ratio")]
    Ratio(TestRatioReport),
    /// Public functions with no direct test caller
    #[serde(rename = "gaps")]
    Gaps(TestGapsReport),
    /// Line budget breakdown by purpose
    #[serde(rename = "budget")]
    Budget(BudgetReport),
}

impl OutputFormatter for CoverageOutput {
    fn format_text(&self) -> String {
        match self {
            CoverageOutput::Ratio(r) => r.format_text(),
            CoverageOutput::Gaps(r) => r.format_text(),
            CoverageOutput::Budget(r) => r.format_text(),
        }
    }

    fn format_pretty(&self) -> String {
        match self {
            CoverageOutput::Ratio(r) => r.format_pretty(),
            CoverageOutput::Gaps(r) => r.format_pretty(),
            CoverageOutput::Budget(r) => r.format_pretty(),
        }
    }
}
