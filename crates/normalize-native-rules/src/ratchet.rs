//! Ratchet native rule: detect metric regressions against the stored baseline.
//!
//! Reads `.normalize/ratchet.json` and compares the current measured values
//! against the stored baseline, reporting regressions as `DiagnosticsReport` issues.
//!
//! Rule IDs:
//! - `ratchet/complexity-total`        — aggregate complexity regression
//! - `ratchet/complexity-per-function` — per-function complexity regressions

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use normalize_ratchet::Metric;
use normalize_ratchet::baseline;
use normalize_ratchet::check::check_against_baseline;
use normalize_ratchet::complexity::TOTAL_KEY;
use std::path::Path;

/// Run the ratchet check and return a `DiagnosticsReport`.
///
/// `metric_factory` is called to produce the metrics to measure. If the
/// baseline file doesn't exist, the report is empty (no violations).
pub fn build_ratchet_report(
    root: &Path,
    metric_factory: fn() -> Vec<Box<dyn Metric>>,
) -> DiagnosticsReport {
    let stored = match baseline::load(root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("normalize-ratchet: failed to load baseline: {e}");
            return DiagnosticsReport::new();
        }
    };

    // If baseline is empty (no metrics), nothing to check
    if stored.metrics.is_empty() {
        return DiagnosticsReport::new();
    }

    let metrics = metric_factory();
    let metric_refs: Vec<&dyn Metric> = metrics.iter().map(|m| m.as_ref()).collect();

    let mut measurements = std::collections::HashMap::new();
    for metric in &metrics {
        match metric.measure(root) {
            Ok(entries) => {
                measurements.insert(metric.name().to_string(), entries);
            }
            Err(e) => {
                eprintln!(
                    "normalize-ratchet: failed to measure {}: {e}",
                    metric.name()
                );
                return DiagnosticsReport::new();
            }
        }
    }

    let result = check_against_baseline(&stored, &measurements, &metric_refs);

    let mut issues: Vec<Issue> = result
        .regressions
        .into_iter()
        .map(|r| {
            let (rule_id, file) = if r.key == TOTAL_KEY {
                (
                    "ratchet/complexity-total".to_string(),
                    ".normalize/ratchet.json".to_string(),
                )
            } else {
                (
                    "ratchet/complexity-per-function".to_string(),
                    r.key.clone(),
                )
            };
            Issue {
                file,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id,
                message: format!(
                    "{}/{}: complexity {} -> {} (delta +{})",
                    r.metric, r.key, r.baseline, r.current, r.delta
                ),
                severity: Severity::Error,
                source: "ratchet".into(),
                related: vec![],
                suggestion: Some(
                    "reduce complexity or run `normalize ratchet update --force` to raise the baseline"
                        .into(),
                ),
            }
        })
        .collect();

    let keys_checked = result.keys_checked;
    let mut report = DiagnosticsReport::new();
    report.issues.append(&mut issues);
    report.files_checked = keys_checked;
    report.sources_run.push("ratchet".into());
    report
}
