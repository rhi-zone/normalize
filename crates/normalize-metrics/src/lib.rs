//! Code quality metrics: cyclomatic complexity, function length, test gap analysis.
//!
//! Provides tree-sitter-backed analyzers that work on source text and produce
//! structured reports. All report types implement `OutputFormatter` for
//! consistent text/pretty/JSON output.

pub mod complexity;
pub mod function_length;
pub mod test_gaps;

use serde::Serialize;

// Re-exports for convenience
pub use complexity::FunctionComplexity;
pub use function_length::FunctionLength;

/// Generic report for file-level analysis (shared by complexity and length).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileReport<T: Serialize + schemars::JsonSchema> {
    pub functions: Vec<T>,
    pub file_path: String,
    /// Stats computed before limit was applied (for accurate reporting when limited).
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub full_stats: Option<FullStats>,
    /// Git ref used as baseline for diff (set when `--diff` is used).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diff_ref: Option<String>,
}

/// Statistics computed on the full result set before limiting.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FullStats {
    pub total_count: usize,
    pub total_avg: f64,
    pub total_max: usize,
    pub critical_count: usize,
    pub high_count: usize,
}
