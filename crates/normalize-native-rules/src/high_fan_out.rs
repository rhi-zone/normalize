//! `high-fan-out` native rule — flags files that import from too many other files.
//!
//! High fan-out is a structural design smell: a file coupled to many others becomes
//! a change magnet and makes the system harder to reason about in isolation.
//!
//! Requires the structural index (`normalize structure rebuild`).
//!
//! # Configuration
//!
//! ```toml
//! [rules.rule."high-fan-out"]
//! threshold = 15   # default: 20
//! ```

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity, ToolFailure};
use std::path::Path;

/// Build a `DiagnosticsReport` for the `high-fan-out` rule.
///
/// Opens the structural index under `root`, queries fan-out counts per file, and
/// emits a warning for each file that imports from strictly more than `threshold`
/// distinct resolved files.
pub async fn build_high_fan_out_report(root: &Path, threshold: usize) -> DiagnosticsReport {
    let mut report = DiagnosticsReport::new();

    let db_path = crate::check_refs::normalize_dir_for_root(root).join("index.sqlite");
    let idx = match normalize_facts::FileIndex::open(&db_path, root).await {
        Ok(idx) => idx,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "high-fan-out".into(),
                message: format!(
                    "failed to open index at {}: {}. Run `normalize structure rebuild` first.",
                    db_path.display(),
                    e
                ),
            });
            return report;
        }
    };

    let fan_out = match idx.import_fan_out_by_file().await {
        Ok(v) => v,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "high-fan-out".into(),
                message: format!("failed to query imports table: {e}"),
            });
            return report;
        }
    };

    for (file, count) in &fan_out {
        if *count > threshold {
            report.issues.push(Issue {
                file: file.clone(),
                line: Some(1),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "high-fan-out".into(),
                message: format!(
                    "file imports from {count} modules (threshold: {threshold})"
                ),
                severity: Severity::Warning,
                source: "high-fan-out".into(),
                related: vec![],
                suggestion: Some(
                    "consider splitting responsibilities or introducing an abstraction layer to reduce coupling".into(),
                ),
            });
        }
    }

    report.files_checked = fan_out.len();
    report.sources_run.push("high-fan-out".into());
    report
}
