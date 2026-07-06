//! Per-file complexity helper for hotspot scoring.
//!
//! Reproduces `ComplexityAnalyzer`'s MAX-per-function semantic without depending
//! on the main crate: parse the file, walk the tags query to find
//! function/method definitions, compute complexity for each via
//! `normalize_facts::extract::compute_complexity`, and return the maximum.

use normalize_facts::extract::compute_complexity;
use normalize_languages::parsers;
use normalize_languages::support_for_path;
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// Compute the maximum cyclomatic complexity across all functions/methods in a
/// file, matching `ComplexityAnalyzer::analyze(...).functions.iter().map(|f|
/// f.complexity).max()`.
///
/// Returns `None` when the language is unsupported, parsing fails, or the file
/// contains no named function/method definitions.
pub fn max_function_complexity(path: &Path, content: &str) -> Option<usize> {
    let support = support_for_path(path)?;
    let grammar_name = support.grammar_name();
    let tree = parsers::parse_with_grammar(grammar_name, content)?;
    let loader = parsers::grammar_loader();

    let tags_scm = loader.get_tags(grammar_name)?;
    let ts_lang = loader.get(grammar_name).ok()?;
    let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;
    let capture_names = tags_query.capture_names();

    let root = tree.root_node();
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(&tags_query, root, content.as_bytes());

    let mut max: Option<usize> = None;
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let cn = capture_names[capture.index as usize];
            if !matches!(cn, "definition.function" | "definition.method") {
                continue;
            }
            // Mirror ComplexityAnalyzer: only count functions with a resolvable name.
            if support.node_name(&capture.node, content).is_none() {
                continue;
            }
            let complexity = compute_complexity(&capture.node, support, content.as_bytes());
            max = Some(max.map_or(complexity, |m| m.max(complexity)));
        }
    }
    max
}
