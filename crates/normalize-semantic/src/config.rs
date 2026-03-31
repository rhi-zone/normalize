//! Configuration for the semantic embeddings subsystem.
//!
//! Added to `NormalizeConfig` under the `[embeddings]` key:
//!
//! ```toml
//! [embeddings]
//! enabled = true
//! model = "nomic-embed-text-v1.5"
//! ```

use crate::embedder::DEFAULT_MODEL;
use serde::{Deserialize, Serialize};

#[cfg(feature = "cli")]
use schemars::JsonSchema;

/// Embeddings configuration (`[embeddings]` section of `.normalize/config.toml`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
#[serde(default)]
pub struct EmbeddingsConfig {
    /// Whether semantic embeddings are enabled. Defaults to false.
    pub enabled: bool,
    /// Embedding model to use. Changing this triggers a full re-embed.
    pub model: String,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: DEFAULT_MODEL.to_string(),
        }
    }
}
