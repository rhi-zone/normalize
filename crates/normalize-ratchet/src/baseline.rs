//! Baseline file format and config for the ratchet system.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Aggregation strategy for reducing multiple values to one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Aggregate {
    #[default]
    Mean,
    Median,
    Max,
    Min,
    Sum,
    Count,
}

impl std::str::FromStr for Aggregate {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mean" => Ok(Aggregate::Mean),
            "median" => Ok(Aggregate::Median),
            "max" => Ok(Aggregate::Max),
            "min" => Ok(Aggregate::Min),
            "sum" => Ok(Aggregate::Sum),
            "count" => Ok(Aggregate::Count),
            other => Err(format!(
                "unknown aggregation '{other}'; expected mean|median|max|min|sum|count"
            )),
        }
    }
}

impl std::fmt::Display for Aggregate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Aggregate::Mean => "mean",
            Aggregate::Median => "median",
            Aggregate::Max => "max",
            Aggregate::Min => "min",
            Aggregate::Sum => "sum",
            Aggregate::Count => "count",
        };
        f.write_str(s)
    }
}

/// A single baseline entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub path: String,
    pub metric: String,
    pub aggregate: Aggregate,
    pub value: f64,
}

/// The full baseline file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineFile {
    pub version: u32,
    pub entries: Vec<BaselineEntry>,
}

impl Default for BaselineFile {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

/// Path to the ratchet baseline file.
pub fn baseline_path(root: &Path) -> PathBuf {
    root.join(".normalize").join("ratchet.json")
}

/// Load the baseline file, returning default if not found.
pub fn load_baseline(root: &Path) -> anyhow::Result<BaselineFile> {
    let path = baseline_path(root);
    if !path.exists() {
        return Ok(BaselineFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))
}

/// Save the baseline file.
pub fn save_baseline(root: &Path, file: &BaselineFile) -> anyhow::Result<()> {
    let path = baseline_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(file)?;
    std::fs::write(&path, json + "\n")
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
}

/// Compute an aggregated value from a list of measurements.
pub fn aggregate(values: &mut [f64], strategy: Aggregate) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(match strategy {
        Aggregate::Mean => values.iter().sum::<f64>() / values.len() as f64,
        Aggregate::Median => {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = values.len() / 2;
            if values.len().is_multiple_of(2) {
                (values[mid - 1] + values[mid]) / 2.0
            } else {
                values[mid]
            }
        }
        Aggregate::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Aggregate::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        Aggregate::Sum => values.iter().sum(),
        Aggregate::Count => values.len() as f64,
    })
}

// --- Config types ---

/// Per-metric override in `[ratchet.metrics.<name>]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct RatchetConfigMetric {
    /// Default aggregation for this metric.
    pub default_aggregate: Option<Aggregate>,
}

/// Ratchet section of the config (`[ratchet]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct RatchetConfig {
    /// Default aggregation used when no entry-specific aggregate is given.
    pub default_aggregate: Option<Aggregate>,
    /// Per-metric configuration.
    pub metrics: std::collections::HashMap<String, RatchetConfigMetric>,
}

impl RatchetConfig {
    /// Resolve the effective aggregation for a given metric name.
    /// Priority: per-metric override > global default > hardcoded fallback (mean).
    pub fn effective_aggregate(&self, metric: &str) -> Aggregate {
        if let Some(Some(a)) = self.metrics.get(metric).map(|m| m.default_aggregate) {
            return a;
        }
        self.default_aggregate.unwrap_or(Aggregate::Mean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_mean() {
        let mut v = vec![1.0, 2.0, 3.0];
        assert_eq!(aggregate(&mut v, Aggregate::Mean), Some(2.0));
    }

    #[test]
    fn test_aggregate_median_odd() {
        let mut v = vec![3.0, 1.0, 2.0];
        assert_eq!(aggregate(&mut v, Aggregate::Median), Some(2.0));
    }

    #[test]
    fn test_aggregate_median_even() {
        let mut v = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(aggregate(&mut v, Aggregate::Median), Some(2.5));
    }

    #[test]
    fn test_aggregate_max() {
        let mut v = vec![1.0, 5.0, 3.0];
        assert_eq!(aggregate(&mut v, Aggregate::Max), Some(5.0));
    }

    #[test]
    fn test_aggregate_count() {
        let mut v = vec![1.0, 2.0, 3.0];
        assert_eq!(aggregate(&mut v, Aggregate::Count), Some(3.0));
    }

    #[test]
    fn test_aggregate_empty() {
        let mut v: Vec<f64> = vec![];
        assert_eq!(aggregate(&mut v, Aggregate::Mean), None);
    }
}
