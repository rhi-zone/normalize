//! `high-complexity` native rule — flags functions exceeding a cyclomatic complexity threshold.
//!
//! Uses tree-sitter tags queries to identify functions and complexity queries
//! (or the `compute_complexity` fallback) to measure cyclomatic complexity.

use normalize_facts::extract::compute_complexity;
use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::support_for_path;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use rayon::prelude::*;
use std::path::Path;
use streaming_iterator::StreamingIterator;

use crate::walk::gitignore_walk;

/// Default cyclomatic complexity threshold.
const DEFAULT_THRESHOLD: usize = 20;

/// Analyze a single file for high-complexity functions.
///
/// Returns a vec of (rel_path, function_name, start_line, complexity) tuples.
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

    let complexity_query = loader.get_complexity(grammar_name).and_then(|scm| {
        let grammar = loader.get(grammar_name).ok()?;
        tree_sitter::Query::new(&grammar, &scm).ok()
    });

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

            let complexity = if let Some(ref cq) = complexity_query {
                count_complexity_with_query(&node, cq, &content)
            } else {
                compute_complexity(&node, support, content.as_bytes())
            };

            if complexity >= threshold {
                let start_line = node.start_position().row + 1;
                results.push((rel_path.clone(), name, start_line, complexity));
            }
        }
    }

    results
}

/// Count complexity using a tree-sitter query with `@complexity` captures.
fn count_complexity_with_query(
    node: &tree_sitter::Node,
    query: &tree_sitter::Query,
    content: &str,
) -> usize {
    let complexity_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "complexity");

    let Some(complexity_idx) = complexity_idx else {
        return 1;
    };

    let mut qcursor = tree_sitter::QueryCursor::new();
    qcursor.set_byte_range(node.byte_range());

    let mut complexity = 1usize;
    let mut matches = qcursor.matches(query, *node, content.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index as usize == complexity_idx {
                complexity += 1;
            }
        }
    }
    complexity
}

/// Build a `DiagnosticsReport` for the `high-complexity` rule.
///
/// Walks all source files under `root`, parses each with tree-sitter, and emits
/// an issue for every function whose cyclomatic complexity meets or exceeds the
/// threshold.
pub fn build_high_complexity_report(root: &Path) -> DiagnosticsReport {
    let threshold = DEFAULT_THRESHOLD;

    // Collect files first so we can process in parallel.
    let files: Vec<_> = gitignore_walk(root)
        .filter(|e| e.path().is_file())
        .filter(|e| support_for_path(e.path()).is_some())
        .map(|e| e.path().to_path_buf())
        .collect();

    let files_checked = files.len();

    let all_findings: Vec<(String, String, usize, usize)> = files
        .par_iter()
        .flat_map(|path| analyze_file(path, root, threshold))
        .collect();

    let mut issues: Vec<Issue> = all_findings
        .into_iter()
        .map(|(file, name, line, complexity)| Issue {
            file,
            line: Some(line),
            column: None,
            end_line: None,
            end_column: None,
            rule_id: "high-complexity".into(),
            message: format!(
                "function `{name}` has cyclomatic complexity {complexity} (threshold: {threshold})"
            ),
            severity: Severity::Warning,
            source: "high-complexity".into(),
            related: vec![],
            suggestion: Some("consider extracting helper functions to reduce complexity".into()),
        })
        .collect();

    // Sort by complexity descending.
    issues.sort_by(|a, b| {
        // Extract complexity from message for sorting.
        let extract = |msg: &str| -> usize {
            msg.rsplit("complexity ")
                .next()
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        extract(&b.message).cmp(&extract(&a.message))
    });

    DiagnosticsReport {
        issues,
        files_checked,
        sources_run: vec!["high-complexity".into()],
        tool_errors: vec![],
    }
}
