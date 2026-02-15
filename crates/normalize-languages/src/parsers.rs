//! Tree-sitter parser singleton and convenience functions.
//!
//! Provides a global `GrammarLoader` singleton so that grammars are loaded once
//! and shared across all call sites. This is the canonical way to parse source
//! code with tree-sitter in the normalize ecosystem.
//!
//! # Lifetime Safety
//!
//! The singleton is stored in a `'static OnceLock`, so the backing shared
//! libraries are never unloaded. This satisfies the lifetime requirement
//! documented in [`GrammarLoader`].

use crate::GrammarLoader;
use std::sync::{Arc, OnceLock};
use tree_sitter::Parser;

/// Global grammar loader singleton — avoids reloading grammars for each parse.
static GRAMMAR_LOADER: OnceLock<Arc<GrammarLoader>> = OnceLock::new();

/// Get the global grammar loader singleton.
pub fn grammar_loader() -> Arc<GrammarLoader> {
    GRAMMAR_LOADER
        .get_or_init(|| Arc::new(GrammarLoader::new()))
        .clone()
}

/// Create a parser for a specific grammar.
///
/// The grammar name should match tree-sitter grammar names
/// (e.g., "python", "rust", "typescript").
pub fn parser_for(grammar: &str) -> Option<Parser> {
    let language = grammar_loader().get(grammar)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    Some(parser)
}

/// Parse source code with a specific grammar.
///
/// The grammar name should match tree-sitter grammar names
/// (e.g., "python", "rust", "typescript").
pub fn parse_with_grammar(grammar: &str, source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = parser_for(grammar)?;
    parser.parse(source, None)
}

/// List grammars available in external search paths.
pub fn available_external_grammars() -> Vec<String> {
    grammar_loader().available_external()
}
