//! Tree-sitter and ast-grep query support for code search.
//!
//! Supports two pattern syntaxes (auto-detected):
//! - Tree-sitter S-expression: `(call_expression function: (identifier) @fn)`
//! - ast-grep pattern: `$FN($ARGS)` (more human-friendly)

use crate::filter::Filter;
use crate::parsers::grammar_loader;
use normalize_languages::support_for_path;
use normalize_syntax_rules::{MatchResult, is_sexp_pattern, run_astgrep_query, run_sexp_query};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Collect files to search based on path argument.
fn collect_files(path: Option<&Path>, filter: Option<&Filter>) -> Vec<PathBuf> {
    let root = path.unwrap_or(Path::new("."));

    if root.is_file() {
        return vec![root.to_path_buf()];
    }

    let mut files = Vec::new();
    collect_files_recursive(root, filter, &mut files);
    files
}

fn collect_files_recursive(dir: &Path, filter: Option<&Filter>, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip hidden and common non-source directories
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "vendor"
            {
                continue;
            }
            collect_files_recursive(&path, filter, files);
        } else if path.is_file() {
            // Only include files we have language support for
            let matches_filter = filter.map(|f| f.matches(&path)).unwrap_or(true);
            if support_for_path(&path).is_some() && matches_filter {
                files.push(path);
            }
        }
    }
}

/// Run a query and return results without printing (for service layer).
pub fn run_query_service(
    pattern: &str,
    path: Option<&std::path::Path>,
    _show_source: bool,
    _context_lines: usize,
    root: &std::path::Path,
    filter: Option<&crate::filter::Filter>,
) -> Result<Vec<MatchResult>, String> {
    let is_sexp = is_sexp_pattern(pattern);
    let loader = grammar_loader();

    // If path is provided use it, otherwise use root
    let search_path = path.unwrap_or(root);
    let files = collect_files(Some(search_path), filter);

    if files.is_empty() {
        return Ok(Vec::new());
    }

    let mut by_grammar: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        if let Some(lang) = support_for_path(&file) {
            by_grammar
                .entry(lang.grammar_name().to_string())
                .or_default()
                .push(file);
        }
    }

    let mut all_results = Vec::new();

    for (grammar_name, files) in by_grammar {
        let Some(grammar) = loader.get(&grammar_name) else {
            continue;
        };

        for file in files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let results = if is_sexp {
                run_sexp_query(&file, &content, pattern, &grammar, &grammar_name)
            } else {
                run_astgrep_query(&file, &content, pattern, &grammar, &grammar_name)
            };

            match results {
                Ok(r) => all_results.extend(r),
                Err(e) => {
                    if e.contains("Invalid query") || e.contains("Pattern error") {
                        return Err(e);
                    }
                    // Skip per-file errors silently
                }
            }
        }
    }

    Ok(all_results)
}
