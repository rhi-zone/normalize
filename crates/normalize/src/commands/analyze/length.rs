//! Function length analysis - find long functions in codebase

use crate::analyze::function_length::{
    FunctionLength, LengthAnalyzer, LengthCategory, LengthReport,
};
use crate::filter::Filter;
use crate::path_resolve;
use normalize_analyze::ranked::{Scored, rank_pipeline};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use super::collect_code_files;

/// Analyze function lengths in a single file
pub fn analyze_file_length(file_path: &Path) -> Option<LengthReport> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let analyzer = LengthAnalyzer::new();
    Some(analyzer.analyze(file_path, &content))
}

/// Analyze function lengths across a codebase, returning longest functions
pub fn analyze_codebase_length(
    root: &Path,
    limit: usize,
    filter: Option<&Filter>,
    allowlist: &[String],
) -> LengthReport {
    let all_files = path_resolve::all_files(root);
    let code_files = collect_code_files(&all_files, filter);

    let all_functions: Vec<FunctionLength> = code_files
        .par_iter()
        .filter_map(|file| {
            let path = root.join(&file.path);
            let content = std::fs::read_to_string(&path).ok()?;
            let analyzer = LengthAnalyzer::new();
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

    // Filter by allowlist
    let filtered: Vec<_> = if allowlist.is_empty() {
        all_functions
    } else {
        all_functions
            .into_iter()
            .filter(|f| {
                let key = f.qualified_name();
                !allowlist.iter().any(|a| key.contains(a))
            })
            .collect()
    };

    // Wrap in Scored for rank_pipeline
    let mut scored: Vec<Scored<_>> = filtered
        .into_iter()
        .map(|f| {
            let score = f.lines as f64;
            Scored::new(f, score)
        })
        .collect();

    // Count categories before pipeline truncates
    let critical_count = scored
        .iter()
        .filter(|s| s.entity.category() == LengthCategory::TooLong)
        .count();
    let high_count = scored
        .iter()
        .filter(|s| s.entity.category() == LengthCategory::Long)
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

    LengthReport {
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
pub fn apply_length_diff(report: &mut LengthReport, baseline: &LengthReport, diff_ref: &str) {
    // Key: (file_path, parent, name) → baseline line count
    let baseline_map: HashMap<(Option<String>, Option<String>, String), usize> = baseline
        .functions
        .iter()
        .map(|f| {
            (
                (f.file_path.clone(), f.parent.clone(), f.name.clone()),
                f.lines,
            )
        })
        .collect();

    for f in &mut report.functions {
        let key = (f.file_path.clone(), f.parent.clone(), f.name.clone());
        f.delta = Some(match baseline_map.get(&key) {
            Some(&base) => f.lines as i64 - base as i64,
            None => f.lines as i64, // new function — full length as delta
        });
    }

    // Sort by |delta| descending; secondary sort by lines descending
    report.functions.sort_by(|a, b| {
        let da = a.delta.unwrap_or(0).unsigned_abs();
        let db = b.delta.unwrap_or(0).unsigned_abs();
        db.cmp(&da).then(b.lines.cmp(&a.lines))
    });

    report.diff_ref = Some(diff_ref.to_string());
}
