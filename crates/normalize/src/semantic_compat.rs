//! Compatibility shims for semantic-search report types.
//!
//! When the `embeddings` cargo feature is enabled, these are simple
//! re-exports of the real types from `normalize-semantic`. When the
//! feature is disabled (e.g. musl builds without ONNX Runtime
//! prebuilts), structurally-compatible stubs are provided so that
//! the CLI surface (`normalize structure search`,
//! `normalize context --semantic`) still type-checks and returns a
//! clear runtime error explaining the missing capability.

#[cfg(feature = "embeddings")]
pub use normalize_semantic::service::{ContextSearchReport, SearchReport};

#[cfg(not(feature = "embeddings"))]
mod stub {
    use crate::output::OutputFormatter;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// Stub `SearchReport` used when the `embeddings` feature is disabled.
    /// The real type lives in `normalize-semantic::service`.
    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SearchReport {
        pub query: String,
        pub model: String,
        pub results: Vec<()>,
        pub total_scanned: usize,
        pub ann_used: bool,
    }

    impl OutputFormatter for SearchReport {
        fn format_text(&self) -> String {
            "semantic search is not available in this build (compiled without the `embeddings` feature)".to_string()
        }
    }

    /// Stub `ContextSearchReport` used when the `embeddings` feature is disabled.
    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ContextSearchReport {
        pub query: String,
        pub model: String,
        pub results: Vec<()>,
        pub total_scanned: usize,
    }

    impl OutputFormatter for ContextSearchReport {
        fn format_text(&self) -> String {
            "semantic search is not available in this build (compiled without the `embeddings` feature)".to_string()
        }
    }
}

#[cfg(not(feature = "embeddings"))]
pub use stub::{ContextSearchReport, SearchReport};
