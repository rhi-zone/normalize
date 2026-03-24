//! Error types for the budget system.

use thiserror::Error;

/// Errors that can occur in the budget system.
#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("metric '{name}' not found")]
    MetricNotFound { name: String },
    #[error("failed to read budget file: {0}")]
    BudgetRead(#[from] std::io::Error),
    #[error("failed to parse budget file: {0}")]
    BudgetParse(#[from] serde_json::Error),
    #[error("measurement failed for metric '{metric}' at '{path}': {message}")]
    MeasurementFailed {
        metric: String,
        path: String,
        message: String,
    },
    #[error("git operation failed: {0}")]
    GitFailed(String),
}
