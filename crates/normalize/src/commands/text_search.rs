//! Text search configuration.
//!
//! The text search command (`grep`) is implemented in the server-less service layer
//! (`service.rs`). This module retains only the configuration type used by
//! `NormalizeConfig`.

use normalize_derive::Merge;
use serde::Deserialize;

/// Text search command configuration.
#[derive(Debug, Clone, Deserialize, serde::Serialize, Default, Merge, schemars::JsonSchema)]
#[serde(default)]
pub struct TextSearchConfig {
    /// Default maximum number of matches
    pub limit: Option<usize>,
    /// Case-insensitive search by default
    pub ignore_case: Option<bool>,
}

impl TextSearchConfig {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(100)
    }

    pub fn ignore_case(&self) -> bool {
        self.ignore_case.unwrap_or(false)
    }
}
