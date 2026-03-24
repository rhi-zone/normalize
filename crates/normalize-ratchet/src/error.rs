use thiserror::Error;

/// Errors produced by the ratchet system.
#[derive(Debug, Error)]
pub enum RatchetError {
    /// The requested metric name is not registered in the metric factory.
    #[error("metric '{name}' not found")]
    MetricNotFound { name: String },
    /// The baseline file does not exist at the expected path.
    #[error("baseline not found at {path}")]
    BaselineNotFound { path: std::path::PathBuf },
    /// The baseline file could not be read from disk.
    #[error("failed to read baseline: {0}")]
    BaselineRead(#[from] std::io::Error),
    /// The baseline file was read successfully but could not be parsed as JSON.
    #[error("failed to parse baseline: {0}")]
    BaselineParse(#[from] serde_json::Error),
    /// Running the metric against the repository or file failed.
    #[error("measurement failed for metric '{metric}' at '{path}': {reason}")]
    MeasurementFailed {
        metric: String,
        path: String,
        reason: String,
    },
}
