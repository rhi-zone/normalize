//! Budget file format and config for the diff-based budget system.

use normalize_metrics::Aggregate;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Limits on change for a single budget entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BudgetLimits {
    /// Maximum number of items added.
    pub added: Option<f64>,
    /// Maximum number of items removed.
    pub removed: Option<f64>,
    /// Maximum total churn (added + removed).
    pub total: Option<f64>,
    /// Maximum net change (added − removed). Can be negative (requires shrinkage).
    pub net: Option<f64>,
}

impl BudgetLimits {
    pub fn is_empty(&self) -> bool {
        self.added.is_none() && self.removed.is_none() && self.total.is_none() && self.net.is_none()
    }
}

/// A single budget entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEntry {
    /// Path prefix to filter (file, directory, or symbol prefix).
    pub path: String,
    /// Metric name (e.g. "lines", "functions", "todos").
    pub metric: String,
    /// Aggregation strategy for combining diff values.
    pub aggregate: Aggregate,
    /// Git ref used as the base for the diff.
    #[serde(rename = "ref")]
    pub base_ref: String,
    /// Configured limits.
    pub limits: BudgetLimits,
}

/// The full budget file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetFile {
    pub version: u32,
    pub entries: Vec<BudgetEntry>,
}

impl Default for BudgetFile {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

/// Path to the budget file.
pub fn budget_path(root: &Path) -> PathBuf {
    root.join(".normalize").join("budget.json")
}

/// Load the budget file, returning default if not found.
pub fn load_budget(root: &Path) -> anyhow::Result<BudgetFile> {
    let path = budget_path(root);
    if !path.exists() {
        return Ok(BudgetFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))
}

/// Save the budget file.
pub fn save_budget(root: &Path, file: &BudgetFile) -> anyhow::Result<()> {
    let path = budget_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(file)?;
    std::fs::write(&path, json + "\n")
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
}

// --- Config types ---

/// Budget section of the config (`[budget]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct BudgetConfig {
    /// Default git ref when none is specified in an entry or on the CLI.
    pub default_ref: Option<String>,
    /// Default aggregation strategy.
    pub default_aggregate: Option<Aggregate>,
}

impl BudgetConfig {
    pub fn effective_ref(&self) -> String {
        self.default_ref
            .clone()
            .unwrap_or_else(|| "HEAD".to_string())
    }

    pub fn effective_aggregate(&self) -> Aggregate {
        self.default_aggregate.unwrap_or(Aggregate::Sum)
    }
}
