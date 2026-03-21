//! Ratchet service adapter for the normalize binary.
//!
//! Re-exports [`normalize_ratchet::service::RatchetService`] and provides
//! the concrete [`MetricFactory`] that wires in `normalize`'s own complexity
//! analysis.

use std::path::Path;

use anyhow::Result;
use normalize_ratchet::Metric;
use normalize_ratchet::complexity::{ComplexityMetric, TOTAL_KEY, function_key};

/// Measure complexity across `root` using `normalize`'s own analysis engine.
///
/// Returns `(key, value)` pairs suitable for the ratchet baseline:
/// - Per-function: `file/Parent/name` or `file/name`
/// - Aggregate: `::total`
pub fn measure_complexity(root: &Path) -> Result<Vec<(String, i64)>> {
    use crate::commands::analyze::complexity::analyze_codebase_complexity;

    let report = analyze_codebase_complexity(root, usize::MAX, None, None, &[]);

    let mut entries: Vec<(String, i64)> = report
        .functions
        .iter()
        .filter_map(|f| {
            let file = f.file_path.as_deref()?;
            let key = function_key(file, f.parent.as_deref(), &f.name);
            Some((key, f.complexity as i64))
        })
        .collect();

    let total: i64 = entries.iter().map(|(_, v)| v).sum();
    entries.push((TOTAL_KEY.to_string(), total));
    Ok(entries)
}

/// Build the set of metrics used by the ratchet service.
pub fn default_metrics() -> Vec<Box<dyn Metric>> {
    vec![Box::new(ComplexityMetric::new(measure_complexity))]
}

/// The ratchet service, pre-configured with normalize's built-in metrics.
pub type RatchetService = normalize_ratchet::service::RatchetService;

/// Construct a [`RatchetService`] using the default metric factory.
pub fn new_ratchet_service() -> RatchetService {
    RatchetService::new(default_metrics)
}
