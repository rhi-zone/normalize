//! Generic tree-sitter and ast-grep query execution.
//!
//! Provides low-level query runners that operate on a single file's content.
//! Higher-level file discovery and dispatch lives in the CLI tier.

use crate::evaluate_predicates;
use normalize_languages::ast_grep::DynLang;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// Match result from either pattern type.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct MatchResult {
    pub file: PathBuf,
    pub grammar: String,
    pub kind: String,
    pub text: String,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
    pub captures: HashMap<String, String>,
}

/// Detect if pattern is a tree-sitter S-expression (starts with `(`)
/// or an ast-grep pattern (anything else).
pub fn is_sexp_pattern(pattern: &str) -> bool {
    pattern.trim_start().starts_with('(')
}

/// Run a tree-sitter S-expression query against a single file's content.
///
/// Returns one `MatchResult` per capture per match.
pub fn run_sexp_query(
    file: &Path,
    content: &str,
    query_str: &str,
    grammar: &tree_sitter::Language,
    grammar_name: &str,
) -> Result<Vec<MatchResult>, String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(grammar)
        .map_err(|e| format!("Failed to set language: {}", e))?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse file".to_string())?;

    let query =
        tree_sitter::Query::new(grammar, query_str).map_err(|e| format!("Invalid query: {}", e))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches_iter = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut results = Vec::new();
    while let Some(m) = matches_iter.next() {
        if !evaluate_predicates(&query, m, content.as_bytes()) {
            continue;
        }

        for cap in m.captures {
            let node = cap.node;
            let capture_name = query.capture_names()[cap.index as usize].to_string();
            let text = node.utf8_text(content.as_bytes()).unwrap_or("").to_string();

            let mut captures = HashMap::new();
            captures.insert(capture_name.clone(), text.clone());

            results.push(MatchResult {
                file: file.to_path_buf(),
                grammar: grammar_name.to_string(),
                kind: node.kind().to_string(),
                text,
                start_row: node.start_position().row + 1,
                start_col: node.start_position().column + 1,
                end_row: node.end_position().row + 1,
                end_col: node.end_position().column + 1,
                captures,
            });
        }
    }

    Ok(results)
}

/// Run an ast-grep pattern query against a single file's content.
pub fn run_astgrep_query(
    file: &Path,
    content: &str,
    pattern_str: &str,
    grammar: &tree_sitter::Language,
    grammar_name: &str,
) -> Result<Vec<MatchResult>, String> {
    use ast_grep_core::tree_sitter::LanguageExt;

    let lang = DynLang::new(grammar.clone());
    let grep = lang.ast_grep(content);
    let pattern = lang
        .pattern(pattern_str)
        .map_err(|e| format!("Pattern error: {:?}", e))?;

    let mut results = Vec::new();
    let root = grep.root();
    for node_match in root.find_all(&pattern) {
        let text = node_match.text().to_string();
        let start_pos = node_match.start_pos();
        let end_pos = node_match.end_pos();

        // For ast-grep, captures are in the MetaVarEnv, but extracting them
        // is complex. For now, just report the matched text.
        let captures = HashMap::new();

        results.push(MatchResult {
            file: file.to_path_buf(),
            grammar: grammar_name.to_string(),
            kind: node_match.kind().to_string(),
            text,
            start_row: start_pos.line() + 1,
            start_col: start_pos.column(&node_match) + 1,
            end_row: end_pos.line() + 1,
            end_col: end_pos.column(&node_match) + 1,
            captures,
        });
    }

    Ok(results)
}
