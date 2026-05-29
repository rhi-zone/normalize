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

/// Detect if pattern is a tree-sitter S-expression or an ast-grep pattern.
///
/// Tree-sitter S-expression patterns start with `(` (node pattern) or `[` (top-level
/// alternation). ast-grep patterns are plain source-code fragments (identifiers, keywords,
/// expressions) that don't start with either of these characters.
pub fn is_sexp_pattern(pattern: &str) -> bool {
    let trimmed = pattern.trim_start();
    trimmed.starts_with('(') || trimmed.starts_with('[')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sexp_pattern_paren() {
        assert!(is_sexp_pattern("(identifier) @i"));
        assert!(is_sexp_pattern("  (call_expression) @c"));
    }

    #[test]
    fn test_is_sexp_pattern_bracket_toplevel_alternation() {
        assert!(is_sexp_pattern("[(identifier) (comment)] @x"));
        assert!(is_sexp_pattern(
            "  [(string_literal) (number_literal)] @lit"
        ));
    }

    #[test]
    fn test_is_sexp_pattern_astgrep_not_matched() {
        assert!(!is_sexp_pattern("foo.bar()"));
        assert!(!is_sexp_pattern("let $X = $Y"));
        assert!(!is_sexp_pattern("$F($$$ARGS)"));
    }

    #[test]
    fn test_sexp_toplevel_alternation_returns_matches() {
        use normalize_languages::GrammarLoader;
        use std::path::Path;

        let loader = GrammarLoader::new();
        let grammar = loader.get("rust").expect("rust grammar must be available");

        // A simple Rust source with an identifier and a line comment.
        let content = "// hello\nlet x = 1;\n";
        let path = Path::new("test.rs");

        // Top-level alternation: should match both identifiers and line_comments.
        let results = run_sexp_query(
            path,
            content,
            "[(identifier) @i (line_comment) @c]",
            &grammar,
            "rust",
        )
        .expect("sexp query must not error");

        assert!(
            !results.is_empty(),
            "top-level alternation query must return matches; got 0"
        );

        // Confirm we see both capture kinds.
        let has_ident = results.iter().any(|r| r.captures.contains_key("i"));
        let has_comment = results.iter().any(|r| r.captures.contains_key("c"));
        assert!(has_ident, "expected captures with name 'i' (identifier)");
        assert!(
            has_comment,
            "expected captures with name 'c' (line_comment)"
        );
    }
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
