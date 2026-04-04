//! `long-function` native rule — flags functions exceeding a line count threshold.
//!
//! Uses tree-sitter tags queries to identify function boundaries and measures
//! line span (end_line - start_line + 1).

use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::support_for_path;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use rayon::prelude::*;
use std::path::Path;
use streaming_iterator::StreamingIterator;

use crate::cache::{FindingsCache, file_mtime_nanos};
use crate::walk::gitignore_walk;
use normalize_rules_config::WalkConfig;

/// Serializable tuple for the long-function findings cache.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedEntry {
    rel_path: String,
    name: String,
    start_line: usize,
    line_count: usize,
}

/// Analyze a single file for long functions.
///
/// Returns a vec of (rel_path, function_name, start_line, line_count) tuples.
fn analyze_file(path: &Path, root: &Path, threshold: usize) -> Vec<(String, String, usize, usize)> {
    let support = match support_for_path(path) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let grammar_name = support.grammar_name();
    let tree = match parse_with_grammar(grammar_name, &content) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let loader = grammar_loader();
    let tags_scm = match loader.get_tags(grammar_name) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let ts_lang = match loader.get(grammar_name) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let tags_query = match tree_sitter::Query::new(&ts_lang, &tags_scm) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let capture_names = tags_query.capture_names();
    let root_node = tree.root_node();
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());

    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut results = Vec::new();

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let cn = capture_names[capture.index as usize];
            if !matches!(cn, "definition.function" | "definition.method") {
                continue;
            }

            let node = capture.node;
            let name = match support.node_name(&node, &content) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let start_line = node.start_position().row + 1;
            let end_line = node.end_position().row + 1;
            let line_count = end_line.saturating_sub(start_line) + 1;

            if line_count >= threshold {
                results.push((rel_path.clone(), name, start_line, line_count));
            }
        }
    }

    results
}

/// Build a `DiagnosticsReport` for the `long-function` rule.
///
/// Walks all source files under `root`, parses each with tree-sitter, and emits
/// an issue for every function whose line span meets or exceeds the threshold.
pub fn build_long_function_report(
    root: &Path,
    threshold: usize,
    explicit_files: Option<&[std::path::PathBuf]>,
    walk_config: &WalkConfig,
) -> DiagnosticsReport {
    let files: Vec<_> = if let Some(ef) = explicit_files {
        ef.iter()
            .filter(|p| p.is_file())
            .filter(|p| support_for_path(p).is_some())
            .cloned()
            .collect()
    } else {
        gitignore_walk(root, walk_config)
            .filter(|e| e.path().is_file())
            .filter(|e| support_for_path(e.path()).is_some())
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    let files_checked = files.len();
    let cache = FindingsCache::open(root);
    let config_hash = threshold.to_string();
    const ENGINE: &str = "long-function";

    // Split into cached and uncached files.
    let mut all_findings: Vec<(String, String, usize, usize)> = Vec::new();
    let mut uncached: Vec<std::path::PathBuf> = Vec::new();

    for path in &files {
        let path_key = path.to_string_lossy().to_string();
        let mtime = file_mtime_nanos(path);
        if mtime > 0
            && let Some(json) = cache.get(&path_key, mtime, &config_hash, ENGINE)
        {
            let entries: Vec<CachedEntry> = serde_json::from_str(&json).unwrap_or_default();
            all_findings.extend(
                entries
                    .into_iter()
                    .map(|e| (e.rel_path, e.name, e.start_line, e.line_count)),
            );
            continue;
        }
        uncached.push(path.clone());
    }

    // Process uncached files in parallel, then store results.
    #[allow(clippy::type_complexity)]
    let fresh: Vec<(std::path::PathBuf, Vec<(String, String, usize, usize)>)> = uncached
        .par_iter()
        .map(|path| (path.clone(), analyze_file(path, root, threshold)))
        .collect();

    for (path, findings) in fresh {
        let path_key = path.to_string_lossy().to_string();
        let mtime = file_mtime_nanos(&path);
        if mtime > 0 {
            let entries: Vec<CachedEntry> = findings
                .iter()
                .map(|(rel_path, name, start_line, line_count)| CachedEntry {
                    rel_path: rel_path.clone(),
                    name: name.clone(),
                    start_line: *start_line,
                    line_count: *line_count,
                })
                .collect();
            if let Ok(json) = serde_json::to_string(&entries) {
                cache.put(&path_key, mtime, &config_hash, ENGINE, &json);
            }
        }
        all_findings.extend(findings);
    }

    cache.flush();

    let mut issues: Vec<Issue> = all_findings
        .into_iter()
        .map(|(file, name, line, line_count)| Issue {
            file,
            line: Some(line),
            column: None,
            end_line: None,
            end_column: None,
            rule_id: "long-function".into(),
            message: format!("function `{name}` is {line_count} lines (threshold: {threshold})"),
            severity: Severity::Warning,
            source: "long-function".into(),
            related: vec![],
            suggestion: Some(
                "consider breaking this function into smaller, focused functions".into(),
            ),
        })
        .collect();

    // Sort by line count descending.
    issues.sort_by(|a, b| {
        let extract = |msg: &str| -> usize {
            msg.split(" is ")
                .nth(1)
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        extract(&b.message).cmp(&extract(&a.message))
    });

    DiagnosticsReport {
        issues,
        files_checked,
        sources_run: vec!["long-function".into()],
        tool_errors: vec![],
        daemon_cached: false,
    }
}
