//! Baseline update logic.
//!
//! Two modes:
//! - Default (no `--force`): ratchet — only lower values or add new entries.
//! - `--force`: unconditionally overwrite all values.

use std::collections::HashMap;

use crate::Metric;
use crate::baseline::Baseline;

/// Summary of what changed when updating the baseline.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct UpdateSummary {
    /// Number of new keys added.
    pub added: usize,
    /// Number of keys whose values were lowered (ratchet tightened).
    pub lowered: usize,
    /// Number of keys whose values were raised (only in `--force` mode).
    pub raised: usize,
    /// Number of keys removed (were in baseline, no longer measured).
    pub removed: usize,
    /// Number of keys unchanged.
    pub unchanged: usize,
}

/// Result returned by [`compute_update`].
pub struct UpdateResult {
    /// The updated baseline (ready to be persisted).
    pub baseline: Baseline,
    /// Summary of what changed.
    pub summary: UpdateSummary,
}

/// Compute an updated baseline from `current_measurements`.
///
/// - If `force` is `false` (ratchet mode): only lower values or add new entries.
///   Existing values are never raised; removed keys are kept.
/// - If `force` is `true`: unconditionally replace all values; remove stale keys.
pub fn compute_update(
    baseline: &Baseline,
    current_measurements: &HashMap<String, Vec<(String, i64)>>,
    metrics: &[&dyn Metric],
    force: bool,
) -> UpdateResult {
    let mut new_baseline = baseline.clone();
    let mut summary = UpdateSummary {
        added: 0,
        lowered: 0,
        raised: 0,
        removed: 0,
        unchanged: 0,
    };

    for metric in metrics {
        let name = metric.name();
        let current_entries = match current_measurements.get(name) {
            Some(e) => e,
            None => continue,
        };

        let metric_map = new_baseline.metrics.entry(name.to_string()).or_default();

        if force {
            // Collect removed keys
            for key in metric_map.keys() {
                let still_present = current_entries.iter().any(|(k, _)| k == key);
                if !still_present {
                    summary.removed += 1;
                }
            }
            // Count changes before replacing
            for (k, v) in current_entries {
                match metric_map.get(k.as_str()) {
                    None => summary.added += 1,
                    Some(&old) if *v < old => summary.lowered += 1,
                    Some(&old) if *v > old => summary.raised += 1,
                    Some(_) => summary.unchanged += 1,
                }
            }
            // Replace entire map
            *metric_map = current_entries
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
        } else {
            // Ratchet mode: only lower or add
            for (key, current_val) in current_entries {
                match metric_map.get(key.as_str()) {
                    None => {
                        metric_map.insert(key.clone(), *current_val);
                        summary.added += 1;
                    }
                    Some(&baseline_val) if *current_val < baseline_val => {
                        metric_map.insert(key.clone(), *current_val);
                        summary.lowered += 1;
                    }
                    _ => {
                        summary.unchanged += 1;
                    }
                }
            }
        }
    }

    UpdateResult {
        baseline: new_baseline,
        summary,
    }
}
