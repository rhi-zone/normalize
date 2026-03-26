//! Syntax sub-service for server-less CLI.
//!
//! Covers AST-level operations (ast, query, node-types).
//! Rules management has been lifted to the top-level `rules` service.

use crate::commands::syntax::node_types::NodeTypesReport;
use normalize_syntax_rules::MatchResult;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Tree-sitter AST inspection and query tools.
#[derive(Default)]
pub struct SyntaxService {
    /// True when compact mode is active (set by `ast` before display).
    compact: Cell<bool>,
    /// Cached text representation for AST compact/depth output.
    ast_text: std::cell::RefCell<String>,
}

impl SyntaxService {
    pub fn new() -> Self {
        Self {
            compact: Cell::new(false),
            ast_text: std::cell::RefCell::new(String::new()),
        }
    }

    fn display_ast(&self, v: &serde_json::Value) -> String {
        if self.compact.get() {
            self.ast_text.borrow().clone()
        } else {
            serde_json::to_string_pretty(v).unwrap_or_default()
        }
    }

    fn display_query(&self, results: &[MatchResult]) -> String {
        if results.is_empty() {
            return "0 matches".to_string();
        }
        let mut lines = Vec::with_capacity(results.len());
        for m in results {
            let file = m.file.display();
            // Show one line per capture in the match.
            if m.captures.is_empty() {
                let preview = truncate_text(&m.text, 80);
                lines.push(format!("{}:{}: {}", file, m.start_row, preview));
            } else {
                for (capture_name, capture_text) in &m.captures {
                    let preview = truncate_text(capture_text, 80);
                    lines.push(format!(
                        "{}:{}: @{} = {}",
                        file, m.start_row, capture_name, preview
                    ));
                }
            }
        }
        lines.join("\n")
    }

    fn display_node_types(&self, r: &NodeTypesReport) -> String {
        use normalize_output::OutputFormatter;
        r.format_text()
    }
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    // Collapse newlines to spaces for single-line display.
    let single: String = s.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    if single.chars().count() <= max_chars {
        single
    } else {
        let truncated: String = single.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

#[cli(
    name = "syntax",
    description = "Tree-sitter AST inspection and query tools"
)]
impl SyntaxService {
    /// Show AST structure for a file
    ///
    /// Examples:
    ///   normalize syntax ast src/main.rs             # show full AST for a file
    ///   normalize syntax ast src/main.rs -l 42       # show AST node at line 42
    ///   normalize syntax ast src/main.rs --sexp      # output as S-expression
    ///   normalize syntax ast src/main.rs --depth 3   # show only 3 levels deep
    ///   normalize syntax ast src/main.rs --compact   # show node-type outline, no source text
    #[cli(display_with = "display_ast")]
    pub fn ast(
        &self,
        #[param(positional, help = "File to inspect")] file: String,
        #[param(short = 'l', help = "Show node at specific line")] at_line: Option<usize>,
        #[param(help = "Output as S-expression")] sexp: bool,
        #[param(
            short = 'd',
            help = "Limit tree depth (-1 = unlimited, 0 = root only, default 5)"
        )]
        depth: Option<i32>,
        compact: bool,
    ) -> Result<serde_json::Value, String> {
        let depth_val = depth.unwrap_or(5);
        self.compact.set(compact);

        let file_path = PathBuf::from(&file);
        let (json, text) = crate::commands::analyze::ast::build_ast_output(
            &file_path, at_line, sexp, depth_val, compact,
        )?;
        *self.ast_text.borrow_mut() = text;
        Ok(json)
    }

    /// Run tree-sitter or ast-grep queries against the codebase
    ///
    /// Examples:
    ///   normalize syntax query "(function_item name: (identifier) @name)"   # tree-sitter query
    ///   normalize syntax query "fn $NAME() { $$$BODY }" -p src/             # ast-grep pattern
    ///   normalize syntax query "(call_expression)" --show-source            # show full source matches
    #[cli(display_with = "display_query")]
    pub fn query(
        &self,
        #[param(positional, help = "Query pattern (S-expression or ast-grep)")] pattern: String,
        #[param(short = 'p', help = "Path to search (defaults to root)")] path: Option<String>,
        #[param(help = "Show full source for matches")] show_source: bool,
        #[param(short = 'c', help = "Number of context lines")] context_lines: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<MatchResult>, String> {
        let root_path = path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| root.as_ref().map(PathBuf::from))
            .map_or_else(
                || {
                    std::env::current_dir()
                        .map_err(|e| format!("failed to get working directory: {e}"))
                },
                Ok,
            )?;
        crate::commands::analyze::query::run_query_service(
            &pattern,
            Some(&root_path),
            show_source,
            context_lines.unwrap_or(5),
            &root_path,
            None,
        )
    }

    /// List node kinds and field names for a tree-sitter grammar
    #[cli(display_with = "display_node_types")]
    pub fn node_types(
        &self,
        #[param(positional, help = "Language name (e.g. rust, python, go)")] language: String,
        #[param(help = "Filter types and fields by substring (case-insensitive)")] search: Option<
            String,
        >,
    ) -> Result<NodeTypesReport, String> {
        crate::commands::syntax::node_types::node_types_for_language(&language, search.as_deref())
    }
}
