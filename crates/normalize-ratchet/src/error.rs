use thiserror::Error;

/// Errors produced by the ratchet system.
#[derive(Debug, Error)]
pub enum RatchetError {
    #[error("metric '{name}' not found")]
    MetricNotFound { name: String },
    #[error("baseline not found at {path}")]
    BaselineNotFound { path: std::path::PathBuf },
    #[error("failed to read baseline: {0}")]
    BaselineRead(#[from] std::io::Error),
    #[error("failed to parse baseline: {0}")]
    BaselineParse(#[from] serde_json::Error),
    #[error("measurement failed for metric '{metric}' at '{path}': {reason}")]
    MeasurementFailed {
        metric: String,
        path: String,
        reason: String,
    },
}
