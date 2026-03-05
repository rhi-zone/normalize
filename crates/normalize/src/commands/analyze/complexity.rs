//! Complexity analysis - find complex functions in codebase

use crate::analyze::complexity::{ComplexityAnalyzer, ComplexityReport};
use crate::filter::Filter;
use crate::path_resolve;
use normalize_analyze::ranked::{Scored, rank_pipeline};
use rayon::prelude::*;
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
    }
}
