//! Baseline file format and config for the ratchet system.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Re-export from normalize-metrics so callers don't need to depend on both.
pub use normalize_metrics::{Aggregate, compute_aggregate};

/// A single baseline entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    /// Relative path (or symbol address) this entry tracks.
    pub path: String,
    /// Name of the metric (e.g. "complexity", "line-count").
    pub metric: String,
    /// Aggregation strategy applied when multiple items match the path.
    pub aggregate: Aggregate,
    /// The pinned baseline value.
    pub value: f64,
}

/// The full baseline file (`.normalize/ratchet.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineFile {
    /// File format version (currently `1`).
    pub version: u32,
    /// All tracked baseline entries.
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

impl BaselineFile {
    /// Load the ratchet baseline file from `root/.normalize/ratchet.json`.
    ///
    /// Returns `Ok(None)` when the file does not exist (ratchet not initialised),
    /// `Ok(Some(file))` on success, and `Err` on IO or parse errors.
    pub fn load(root: &Path) -> anyhow::Result<Option<Self>> {
        let path = ratchet_path(root);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        let file = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
        Ok(Some(file))
    }

    /// Write this baseline file to `root/.normalize/ratchet.json`.
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let path = ratchet_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("failed to create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json + "\n")
            .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
    }
}

/// Path to the ratchet baseline file (`<root>/.normalize/ratchet.json`).
pub fn ratchet_path(root: &Path) -> PathBuf {
    root.join(".normalize").join("ratchet.json")
}

// ---------------------------------------------------------------------------
// Deprecated free-function aliases kept for a single transition step.
// ---------------------------------------------------------------------------

/// Load the baseline file, returning default if not found.
#[deprecated(
    since = "0.2.0",
    note = "use BaselineFile::load — unlike this function, it returns None instead of default when the file is absent"
)]
pub fn load_baseline(root: &Path) -> anyhow::Result<BaselineFile> {
    Ok(BaselineFile::load(root)?.unwrap_or_default())
}

/// Save the baseline file.
#[deprecated(since = "0.2.0", note = "use file.save(root)")]
pub fn save_baseline(root: &Path, file: &BaselineFile) -> anyhow::Result<()> {
    file.save(root)
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
