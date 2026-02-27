//! Syntax sub-service for server-less CLI.
//!
//! Covers AST-level operations and syntax rule management.
//! Parallel to `facts` (which covers fact extraction and fact-based rules).

use crate::commands::analyze::query::MatchResult;
use crate::service::rules::RulesService;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Syntax sub-service — AST inspection and syntax rules.
pub struct SyntaxService {
    rules: RulesService,
}

impl SyntaxService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            rules: RulesService::new(pretty),
        }
    }

    fn display_ast(&self, v: &serde_json::Value) -> String {
        serde_json::to_string_pretty(v).unwrap_or_default()
    }

    fn display_query(&self, results: &[MatchResult]) -> String {
        format!("{} matches", results.len())
    }
}

#[cli(name = "syntax", about = "AST inspection and syntax rules")]
impl SyntaxService {
    /// Show AST structure for a file
    #[cli(display_with = "display_ast")]
    pub fn ast(
        &self,
        #[param(positional, help = "File to inspect")] file: String,
        #[param(short = 'l', help = "Show node at specific line")] at_line: Option<usize>,
        #[param(help = "Output as S-expression")] sexp: bool,
    ) -> Result<serde_json::Value, String> {
        let file_path = PathBuf::from(&file);
        let (json, _text) =
            crate::commands::analyze::ast::build_ast_output(&file_path, at_line, sexp)?;
        Ok(json)
    }

    /// Run tree-sitter or ast-grep queries against the codebase
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
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        crate::commands::analyze::query::run_query_service(
            &pattern,
            Some(&root_path),
            show_source,
            context_lines.unwrap_or(5),
            &root_path,
            None,
        )
    }

    /// Manage and run syntax rules
    pub fn rules(&self) -> &RulesService {
        &self.rules
    }
}
