//! Error types for the budget system.

use thiserror::Error;

/// Errors that can occur in the budget system.
#[derive(Debug, Error)]
pub enum BudgetError {
    /// The requested metric name is not registered in the diff metric factory.
    #[error("metric '{name}' not found")]
    MetricNotFound { name: String },
    /// The budget file could not be read from disk.
    #[error("failed to read budget file: {0}")]
    BudgetRead(#[from] std::io::Error),
    /// The budget file was read successfully but could not be parsed as JSON.
    #[error("failed to parse budget file: {0}")]
    BudgetParse(#[from] serde_json::Error),
    /// Running the diff metric against the repository failed.
    #[error("measurement failed for metric '{metric}' at '{path}': {reason}")]
    MeasurementFailed {
        metric: String,
        path: String,
        reason: String,
    },
    /// A git subprocess invocation failed.
    #[error("git operation failed: {0}")]
    GitFailed(String),
}
