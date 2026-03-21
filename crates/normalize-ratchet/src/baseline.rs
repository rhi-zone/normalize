//! Baseline file I/O for `.normalize/ratchet.json`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Current format version for `ratchet.json`.
pub const BASELINE_VERSION: u32 = 1;

/// The baseline file stored at `.normalize/ratchet.json`.
///
/// ```json
/// {
///   "version": 1,
///   "metrics": {
///     "complexity": {
///       "crates/normalize/src/service/analyze.rs/AnalyzeService/health": 6,
///       "::total": 1247
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Baseline {
    /// Format version — currently always 1.
    pub version: u32,
    /// Map from metric name → (key → value).
    pub metrics: HashMap<String, HashMap<String, i64>>,
}

impl Default for Baseline {
    fn default() -> Self {
        Self {
            version: BASELINE_VERSION,
            metrics: HashMap::new(),
        }
    }
}

/// Path of the ratchet baseline file relative to the project root.
pub fn baseline_path(root: &Path) -> std::path::PathBuf {
    root.join(".normalize").join("ratchet.json")
}

/// Load the baseline from `.normalize/ratchet.json`.
///
/// Returns `Ok(Baseline::default())` if the file does not exist.
pub fn load(root: &Path) -> Result<Baseline> {
    let path = baseline_path(root);
    if !path.exists() {
        return Ok(Baseline::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let baseline: Baseline = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    Ok(baseline)
}

/// Persist `baseline` to `.normalize/ratchet.json`, creating the directory if needed.
pub fn save(root: &Path, baseline: &Baseline) -> Result<()> {
    let path = baseline_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(baseline).context("failed to serialize baseline")?;
    std::fs::write(&path, content + "\n")
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
