//! Diff-based budget system for normalize.
//!
//! A budget entry tracks how much a path is allowed to change relative to a base git ref.
//! Each entry has up to four independent optional limits:
//! - `max_added`: items introduced
//! - `max_removed`: items deleted
//! - `max_total`: added + removed (total churn)
//! - `max_net`: added − removed (net growth; can be negative to require shrinkage)

pub mod budget;
pub mod metrics;

#[cfg(feature = "cli")]
pub mod service;

pub use budget::{BudgetConfig, BudgetEntry, BudgetFile, BudgetLimits, budget_path};
pub use metrics::DiffMetric;

/// Factory function type: produce all diff metrics.
/// Lives outside the `cli` feature so `normalize-native-rules` can use it.
pub type DiffMetricFactory = fn() -> Vec<Box<dyn DiffMetric>>;

/// Create the default diff metric registry.
pub fn default_diff_metrics() -> Vec<Box<dyn DiffMetric>> {
    vec![
        Box::new(metrics::lines::LinesMetric),
        Box::new(metrics::functions::FunctionsMetric),
        Box::new(metrics::classes::ClassesMetric),
        Box::new(metrics::modules::ModulesMetric),
        Box::new(metrics::todos::TodosMetric),
        Box::new(metrics::complexity_delta::ComplexityDeltaMetric),
        Box::new(metrics::dependencies::DependenciesMetric),
    ]
}
