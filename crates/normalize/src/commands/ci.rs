//! `normalize ci` — unified CI entry point.
//!
//! Runs all configured rule engines (syntax, native, fact) in sequence,
//! aggregates violations into a single `DiagnosticsReport`, and exits
//! non-zero if any errors are found (or if `--strict` is set and there are
//! warnings).

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, Serializer};

/// Helper struct used for JSON schema and deserialization only.
///
/// Mirrors the serialized shape of `CiReport` (which includes computed count
/// fields). `Deserialize` is derived here so `CiReport` itself does not need
/// to implement it directly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct CiReportSchema {
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

/// Report returned by `normalize ci`.
///
/// Wraps a `DiagnosticsReport` with metadata: which engines were run,
/// total duration, and per-severity counts for quick scanning.
///
/// The `error_count`, `warning_count`, and `info_count` are computed from
/// `diagnostics` on each access rather than stored as fields, so they are
/// always consistent even if `diagnostics` is mutated after construction.
#[derive(Debug, Clone, JsonSchema)]
#[schemars(with = "CiReportSchema")]
pub struct CiReport {
    /// The merged diagnostics from all engines that were run.
    pub diagnostics: DiagnosticsReport,
    /// Which engines were included in this run (e.g. "syntax", "native", "fact").
    pub engines_run: Vec<String>,
    /// Total elapsed time in milliseconds.
    pub duration_ms: u64,
}

impl Serialize for CiReport {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CiReport", 6)?;
        state.serialize_field("diagnostics", &self.diagnostics)?;
        state.serialize_field("engines_run", &self.engines_run)?;
        state.serialize_field("duration_ms", &self.duration_ms)?;
        state.serialize_field("error_count", &self.error_count())?;
        state.serialize_field("warning_count", &self.warning_count())?;
        state.serialize_field("info_count", &self.info_count())?;
        state.end()
    }
}

impl CiReport {
    /// Build a `CiReport` from a finished `DiagnosticsReport` and timing info.
    pub fn new(diagnostics: DiagnosticsReport, engines_run: Vec<String>, duration_ms: u64) -> Self {
        Self {
            diagnostics,
            engines_run,
            duration_ms,
        }
    }

    /// Number of error-severity issues in this report.
    pub fn error_count(&self) -> usize {
        self.diagnostics.count_by_severity(Severity::Error)
    }

    /// Number of warning-severity issues in this report.
    pub fn warning_count(&self) -> usize {
        self.diagnostics.count_by_severity(Severity::Warning)
    }

    /// Number of info-severity issues in this report.
    pub fn info_count(&self) -> usize {
        self.diagnostics.count_by_severity(Severity::Info)
    }
}

impl OutputFormatter for CiReport {
    fn format_text(&self) -> String {
        let diag_text = self.diagnostics.format_text_limited(Some(50));
        let engines = self.engines_run.join(", ");
        let summary = format!(
            "ci: {} error(s), {} warning(s), {} info — engines: {} — {}ms",
            self.error_count(),
            self.warning_count(),
            self.info_count(),
            engines,
            self.duration_ms
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
            self.error_count(),
            self.warning_count(),
            self.info_count(),
            engines,
            self.duration_ms
        );
        if diag_text.trim().is_empty() {
            summary
        } else {
            format!("{diag_text}\n{summary}")
        }
    }
}
