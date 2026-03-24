//! Metric regression-tracking (ratchet) system for normalize.
//!
//! Each baseline entry is a `(path, metric, aggregation) → value` triple stored
//! in `.normalize/ratchet.json`. The framework measures current values, compares
//! to baselines, and reports regressions.

pub mod baseline;
pub mod error;
pub mod metrics;

#[cfg(feature = "cli")]
pub mod service;

pub use baseline::{Aggregate, BaselineEntry, BaselineFile, RatchetConfig, RatchetConfigMetric};
pub use error::RatchetError;
// Re-export Metric and MetricFactory from normalize-metrics for API consumers.
pub use normalize_metrics::{Metric, MetricFactory};

/// Returns the default metric registry: complexity, call-complexity, line-count,
/// function-count, class-count, and comment-line-count.
///
/// The `root` argument is unused; it exists to satisfy the `MetricFactory` signature
/// for future root-aware metric registration.
pub fn default_metrics(
    _root: &std::path::Path, // root is unused here; exists to match MetricFactory<T> = fn(&Path) -> Vec<T>
) -> Vec<Box<dyn Metric>> {
    vec![
        Box::new(metrics::complexity::ComplexityMetric),
        Box::new(metrics::call_complexity::CallComplexityMetric),
        Box::new(metrics::file_stats::LineCountMetric),
        Box::new(metrics::file_stats::FunctionCountMetric),
        Box::new(metrics::file_stats::ClassCountMetric),
        Box::new(metrics::file_stats::CommentLineCountMetric),
    ]
}
