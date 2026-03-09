//! Complexity analysis - find complex functions in codebase

use crate::analyze::complexity::{ComplexityAnalyzer, ComplexityReport};
use crate::filter::Filter;
use crate::path_resolve;
use normalize_analyze::ranked::{Scored, rank_pipeline};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use super::collect_code_files;

/// Analyze complexity of a single file
pub fn analyze_file_complexity(file_path: &Path) -> Option<ComplexityReport> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let analyzer = ComplexityAnalyzer::new();
    Some(analyzer.analyze(file_path, &content))
}

/// Analyze complexity across a codebase, returning top complex functions
pub fn analyze_codebase_complexity(
    root: &Path,
    limit: usize,
    threshold: Option<usize>,
    filter: Option<&Filter>,
    allowlist: &[String],
) -> ComplexityReport {
    let all_files = path_resolve::all_files(root);
    let code_files = collect_code_files(&all_files, filter);

    let all_functions: Vec<_> = code_files
        .par_iter()
        .filter_map(|file| {
            let path = root.join(&file.path);
            let content = std::fs::read_to_string(&path).ok()?;
            let analyzer = ComplexityAnalyzer::new();
            let report = analyzer.analyze(&path, &content);
            Some(
                report
                    .functions
                    .into_iter()
                    .map(|mut f| {
                        f.file_path = Some(file.path.clone());
                        f
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    // Filter by threshold
    let mut filtered: Vec<_> = if let Some(t) = threshold {
        all_functions
            .into_iter()
            .filter(|f| f.complexity >= t)
            .collect()
    } else {
        all_functions
    };

    // Filter by allowlist
    if !allowlist.is_empty() {
        filtered.retain(|f| {
            let key = f.qualified_name();
            !allowlist.iter().any(|a| key.contains(a))
        });
    }

    // Wrap in Scored for rank_pipeline
    let mut scored: Vec<Scored<_>> = filtered
        .into_iter()
        .map(|f| {
            let score = f.complexity as f64;
            Scored::new(f, score)
        })
        .collect();

    // Count categories before pipeline truncates
    let critical_count = scored.iter().filter(|s| s.entity.complexity > 20).count();
    let high_count = scored
        .iter()
        .filter(|s| s.entity.complexity >= 11 && s.entity.complexity <= 20)
        .count();

    let stats = rank_pipeline(&mut scored, limit, false);

    let full_stats = if stats.total_count > 0 {
        Some(crate::analyze::FullStats {
            total_count: stats.total_count,
            total_avg: stats.avg,
            total_max: stats.max as usize,
            critical_count,
            high_count,
        })
    } else {
        None
    };

    let functions = scored.into_iter().map(|s| s.entity).collect();

    ComplexityReport {
        functions,
        file_path: root.to_string_lossy().to_string(),
        full_stats,
        diff_ref: None,
    }
}

/// Annotate `report` with per-function deltas relative to `baseline`.
///
/// After this call, `report.functions` is sorted by `|delta|` descending and
/// `report.diff_ref` is set to the baseline ref string.
pub fn apply_complexity_diff(
    report: &mut ComplexityReport,
    baseline: &ComplexityReport,
    diff_ref: &str,
) {
    // Key: (file_path, parent, name) → baseline complexity
    let baseline_map: HashMap<(Option<String>, Option<String>, String), usize> = baseline
        .functions
        .iter()
        .map(|f| {
            (
                (f.file_path.clone(), f.parent.clone(), f.name.clone()),
                f.complexity,
            )
        })
        .collect();

    for f in &mut report.functions {
        let key = (f.file_path.clone(), f.parent.clone(), f.name.clone());
        f.delta = Some(match baseline_map.get(&key) {
            Some(&base) => f.complexity as i64 - base as i64,
            None => f.complexity as i64, // new function — full complexity as delta
        });
    }

    // Sort by |delta| descending; secondary sort by complexity descending
    report.functions.sort_by(|a, b| {
        let da = a.delta.unwrap_or(0).unsigned_abs();
        let db = b.delta.unwrap_or(0).unsigned_abs();
        db.cmp(&da).then(b.complexity.cmp(&a.complexity))
    });

    report.diff_ref = Some(diff_ref.to_string());
}
