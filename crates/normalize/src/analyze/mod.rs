//! Analysis passes for code quality metrics.

pub mod complexity;
pub mod function_length;

use serde::Serialize;

// Re-exports for convenience
pub use complexity::FunctionComplexity;
pub use function_length::FunctionLength;

/// Generic report for file-level analysis (shared by complexity and length).
#[derive(Debug, Serialize)]
pub struct FileReport<T: Serialize> {
    pub functions: Vec<T>,
    pub file_path: String,
    /// Stats computed before limit was applied (for accurate reporting when limited).
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub full_stats: Option<FullStats>,
}

/// Statistics computed on the full result set before limiting.
#[derive(Debug, Clone, Serialize)]
pub struct FullStats {
    pub total_count: usize,
    pub total_avg: f64,
    pub total_max: usize,
    pub critical_count: usize,
    pub high_count: usize,
}
