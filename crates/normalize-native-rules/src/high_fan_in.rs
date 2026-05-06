//! `high-fan-in` native rule — flags files that are imported by too many other files.
//!
//! High fan-in is a structural design smell: a file with many dependents is a fragile
//! shared dependency — any change to its interface ripples across the codebase.
//!
//! Requires the structural index (`normalize structure rebuild`).
//!
//! # Configuration
//!
//! ```toml
//! [rules.rule."high-fan-in"]
//! threshold = 15   # default: 20
//! ```

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity, ToolFailure};
use std::path::Path;

/// Build a `DiagnosticsReport` for the `high-fan-in` rule.
///
/// Opens the structural index under `root`, queries fan-in counts per file, and
/// emits a warning for each file that is imported by strictly more than `threshold`
/// distinct files.
pub async fn build_high_fan_in_report(root: &Path, threshold: usize) -> DiagnosticsReport {
    let mut report = DiagnosticsReport::new();

    let db_path = crate::check_refs::normalize_dir_for_root(root).join("index.sqlite");
    let idx = match normalize_facts::FileIndex::open(&db_path, root).await {
        Ok(idx) => idx,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "high-fan-in".into(),
                message: format!(
                    "failed to open index at {}: {}. Run `normalize structure rebuild` first.",
                    db_path.display(),
                    e
                ),
            });
            return report;
        }
    };

    let fan_in = match idx.import_fan_in_by_file().await {
        Ok(v) => v,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "high-fan-in".into(),
                message: format!("failed to query imports table: {e}"),
            });
            return report;
        }
    };

    for (file, count) in &fan_in {
        if *count > threshold {
            report.issues.push(Issue {
                file: file.clone(),
                line: Some(1),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "high-fan-in".into(),
                message: format!(
                    "file is imported by {count} modules (threshold: {threshold})"
                ),
                severity: Severity::Warning,
                source: "high-fan-in".into(),
                related: vec![],
                suggestion: Some(
                    "consider whether this file has too many responsibilities or whether callers could reduce their dependency on it".into(),
                ),
            });
        }
    }

    report.files_checked = fan_in.len();
    report.sources_run.push("high-fan-in".into());
    report
}
