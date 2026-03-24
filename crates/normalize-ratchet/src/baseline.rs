//! Baseline file format and config for the ratchet system.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Re-export from normalize-metrics so callers don't need to depend on both.
pub use normalize_metrics::{Aggregate, compute_aggregate};

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
