//! Regression detection: compare current metric values to a stored baseline.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::Metric;
use crate::baseline::Baseline;

/// A single regression — one metric key whose value has gotten worse.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Regression {
    /// Metric name (e.g. `"complexity"`).
    pub metric: String,
    /// Key within the metric (e.g. `"::total"` or `"src/lib.rs/my_fn"`).
    pub key: String,
    /// Value stored in the baseline.
    pub baseline: i64,
    /// Current measured value.
    pub current: i64,
    /// `current - baseline` (positive = got worse for "lower is better" metrics).
    pub delta: i64,
}

/// Result of comparing current measurements to a baseline.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// All detected regressions (non-empty → check failed).
    pub regressions: Vec<Regression>,
    /// Keys that exist in the current measurement but not in the baseline (newly added).
    pub new_keys: Vec<(String, String)>, // (metric, key)
    /// Keys that were in the baseline but are no longer measured (removed/renamed).
    pub removed_keys: Vec<(String, String)>, // (metric, key)
    /// Total number of metric keys compared.
    pub keys_checked: usize,
}

impl CheckResult {
    /// Returns `true` if there are any regressions.
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }
}

/// Compare `current` measurements against `baseline` using each metric's `is_regression` predicate.
pub fn check_against_baseline(
    baseline: &Baseline,
    current_measurements: &HashMap<String, Vec<(String, i64)>>,
    metrics: &[&dyn Metric],
) -> CheckResult {
    let mut regressions = Vec::new();
    let mut new_keys = Vec::new();
    let mut removed_keys = Vec::new();
    let mut keys_checked = 0usize;

    for metric in metrics {
        let name = metric.name();
        let current_entries = match current_measurements.get(name) {
            Some(e) => e,
            None => continue,
        };
        let current_map: HashMap<&str, i64> = current_entries
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();

        let baseline_map: &HashMap<String, i64> = match baseline.metrics.get(name) {
            Some(m) => m,
            None => {
                // Whole metric is new — all keys are "new"
                for (k, _) in current_entries {
                    new_keys.push((name.to_string(), k.clone()));
                }
                continue;
            }
        };

        // Check existing baseline keys
        for (key, &baseline_val) in baseline_map {
            keys_checked += 1;
            match current_map.get(key.as_str()) {
                Some(&current_val) => {
                    if metric.is_regression(baseline_val, current_val) {
                        regressions.push(Regression {
                            metric: name.to_string(),
                            key: key.clone(),
                            baseline: baseline_val,
                            current: current_val,
                            delta: current_val - baseline_val,
                        });
                    }
                }
                None => {
                    removed_keys.push((name.to_string(), key.clone()));
                }
            }
        }

        // New keys not in baseline
        for (k, _) in current_entries {
            if !baseline_map.contains_key(k.as_str()) {
                new_keys.push((name.to_string(), k.clone()));
            }
        }
    }

    CheckResult {
        regressions,
        new_keys,
        removed_keys,
        keys_checked,
    }
}
