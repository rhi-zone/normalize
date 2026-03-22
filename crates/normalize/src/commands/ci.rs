//! `normalize ci` — unified CI entry point.
//!
//! Runs all configured rule engines (syntax, native, fact) in sequence,
//! aggregates violations into a single `DiagnosticsReport`, and exits
//! non-zero if any errors are found (or if `--strict` is set and there are
//! warnings).

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Report returned by `normalize ci`.
///
/// Wraps a `DiagnosticsReport` with metadata: which engines were run,
/// total duration, and per-severity counts for quick scanning.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CiReport {
    /// The merged diagnostics from all engines that were run.
    pub diagnostics: DiagnosticsReport,
    /// Which engines were included in this run (e.g. "syntax", "native", "fact").
    pub engines_run: Vec<String>,
    /// Total elapsed time in milliseconds.
    pub duration_ms: u64,
    /// Number of error-severity issues.
    pub error_count: usize,
    /// Number of warning-severity issues.
    pub warning_count: usize,
    /// Number of info-severity issues.
    pub info_count: usize,
}

impl CiReport {
    /// Build a `CiReport` from a finished `DiagnosticsReport` and timing info.
    pub fn new(diagnostics: DiagnosticsReport, engines_run: Vec<String>, duration_ms: u64) -> Self {
        let error_count = diagnostics.count_by_severity(Severity::Error);
        let warning_count = diagnostics.count_by_severity(Severity::Warning);
        let info_count = diagnostics.count_by_severity(Severity::Info);
        Self {
            diagnostics,
            engines_run,
            duration_ms,
            error_count,
            warning_count,
            info_count,
        }
    }
}

impl OutputFormatter for CiReport {
    fn format_text(&self) -> String {
        let diag_text = self.diagnostics.format_text_limited(Some(50));
        let engines = self.engines_run.join(", ");
        let summary = format!(
            "ci: {} error(s), {} warning(s), {} info — engines: {} — {}ms",
            self.error_count, self.warning_count, self.info_count, engines, self.duration_ms
        );
        if diag_text.trim().is_empty() {
            summary
        } else {
            format!("{diag_text}\n{summary}")
        }
    }

    fn format_pretty(&self) -> String {
        let diag_text = self.diagnostics.format_pretty();
        let engines = self.engines_run.join(", ");
        let summary = format!(
            "ci: {} error(s), {} warning(s), {} info — engines: {} — {}ms",
            self.error_count, self.warning_count, self.info_count, engines, self.duration_ms
        );
        if diag_text.trim().is_empty() {
            summary
        } else {
            format!("{diag_text}\n{summary}")
        }
    }
}
