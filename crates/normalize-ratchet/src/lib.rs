//! Metric regression-tracking (ratchet) system for normalize.
//!
//! Each baseline entry is a `(path, metric, aggregation) → value` triple stored
//! in `.normalize/ratchet.json`. The framework measures current values, compares
//! to baselines, and reports regressions.

pub mod baseline;
pub mod metrics;

#[cfg(feature = "cli")]
pub mod service;

pub use baseline::{Aggregate, BaselineEntry, BaselineFile, RatchetConfig, RatchetConfigMetric};
// Re-export Metric and MetricFactory from normalize-metrics for API consumers.
pub use normalize_metrics::{Metric, MetricFactory};

/// Create the default metric registry.
pub fn default_metrics(_root: &std::path::Path) -> Vec<Box<dyn Metric>> {
    vec![
        Box::new(metrics::complexity::ComplexityMetric),
        Box::new(metrics::call_complexity::CallComplexityMetric),
        Box::new(metrics::file_stats::LineCountMetric),
        Box::new(metrics::file_stats::FunctionCountMetric),
        Box::new(metrics::file_stats::ClassCountMetric),
        Box::new(metrics::file_stats::CommentLineCountMetric),
    ]
}
