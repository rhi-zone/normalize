//! Budget file format and config for the diff-based budget system.

use normalize_metrics::Aggregate;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Limits on change for a single budget entry.
///
/// All fields are optional; unset limits are not checked.
/// Field names match the corresponding CLI flags (`--max-added`, etc.).
///
/// **Note:** The JSON field names changed in v2 (added→max_added, removed→max_removed,
/// total→max_total, net→max_net). Old budget.json files using v1 field names must be
/// migrated manually or regenerated with `normalize budget add`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BudgetLimits {
    /// Maximum number of items added before a violation is raised.
    pub max_added: Option<f64>,
    /// Maximum number of items removed before a violation is raised.
    pub max_removed: Option<f64>,
    /// Maximum total churn (added + removed) before a violation is raised.
    pub max_total: Option<f64>,
    /// Maximum net change (added − removed). Can be negative to require shrinkage.
    pub max_net: Option<f64>,
}

impl BudgetLimits {
    /// Returns true if no limits are configured.
    pub fn is_empty(&self) -> bool {
        self.max_added.is_none()
            && self.max_removed.is_none()
            && self.max_total.is_none()
            && self.max_net.is_none()
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
    /// Schema version. Currently 1.
    pub version: u32,
    /// All tracked budget entries.
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

impl BudgetFile {
    /// Load the budget file from `root/.normalize/budget.json`.
    ///
    /// Returns `Ok(None)` when the file does not exist (budget not initialised),
    /// `Ok(Some(file))` on success, and `Err` on IO or parse errors.
    pub fn load(root: &Path) -> anyhow::Result<Option<Self>> {
        let path = budget_path(root);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        let file = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
        Ok(Some(file))
    }

    /// Write this budget file to `root/.normalize/budget.json`.
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let path = budget_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!("failed to create directory {}: {e}", parent.display())
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json + "\n")
            .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
    }
}

/// Path to the budget file.
pub fn budget_path(root: &Path) -> PathBuf {
    root.join(".normalize").join("budget.json")
}

/// Load the budget file, returning default if not found.
#[deprecated(since = "0.2.0", note = "use BudgetFile::load")]
pub fn load_budget(root: &Path) -> anyhow::Result<BudgetFile> {
    Ok(BudgetFile::load(root)?.unwrap_or_default())
}

/// Write the budget file to `root/.normalize/budget.json`.
#[deprecated(since = "0.2.0", note = "use BudgetFile::save")]
pub fn save_budget(root: &Path, file: &BudgetFile) -> anyhow::Result<()> {
    file.save(root)
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
    /// Returns `default_ref` if set, otherwise `"HEAD"`.
    pub fn effective_ref(&self) -> &str {
        self.default_ref.as_deref().unwrap_or("HEAD")
    }

    /// Returns `default_aggregate` if set, otherwise `Sum`.
    pub fn effective_aggregate(&self) -> Aggregate {
        self.default_aggregate.unwrap_or(Aggregate::Sum)
    }
}
