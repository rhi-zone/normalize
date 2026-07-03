//! Index configuration.
//!
//! This is the `[index]` section of the project config. It lives in
//! `normalize-index` (not the main crate) so that feature crates which only need
//! to acquire the index can depend on this leaf without pulling in the full
//! `NormalizeConfig`. The main crate composes it via `#[param(nested, serde)]`
//! exactly like `WalkConfig` — hence the `serde` + `normalize_core::Merge`
//! derives rather than `server_less::Config`.

use serde::{Deserialize, Serialize};

/// Index configuration (`[index]`).
#[derive(
    Debug, Clone, Deserialize, Serialize, Default, schemars::JsonSchema, normalize_core::Merge,
)]
#[serde(default)]
pub struct IndexConfig {
    /// Whether to create and use the file index. Default: true
    pub enabled: Option<bool>,
}

impl IndexConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}
