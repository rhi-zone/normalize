//! Re-exports from normalize-metrics for backward compatibility within this crate.
//!
//! All types have moved to `normalize_metrics`. This module provides aliases
//! so existing `commands/analyze/` code continues to compile without changes.

pub mod complexity {
    pub use normalize_metrics::complexity::*;
}
pub mod function_length {
    pub use normalize_metrics::function_length::*;
}
pub mod test_gaps {
    pub use normalize_metrics::test_gaps::*;
}

// Re-exports for convenience
pub use normalize_metrics::FileReport;
pub use normalize_metrics::FullStats;
pub use normalize_metrics::FunctionComplexity;
pub use normalize_metrics::FunctionLength;
