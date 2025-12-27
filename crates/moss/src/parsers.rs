//! Tree-sitter parser initialization and management.

use moss_languages::GrammarLoader;
use std::sync::Arc;
use tree_sitter::Parser;

/// Collection of tree-sitter parsers using dynamic grammar loading.
///
/// Grammars are loaded from:
/// 1. `MOSS_GRAMMAR_PATH` environment variable (colon-separated paths)
/// 2. `~/.config/moss/grammars/`
pub struct Parsers {
    loader: Arc<GrammarLoader>,
}

impl Parsers {
    /// Create new parser collection with dynamic grammar loading.
    pub fn new() -> Self {
        Self {
            loader: Arc::new(GrammarLoader::new()),
        }
    }

    /// Create a parser for a specific grammar.
    ///
    /// The grammar name should match tree-sitter grammar names (e.g., "python", "rust", "typescript").
    pub fn parser_for(&self, grammar: &str) -> Option<Parser> {
        let language = self.loader.get(grammar)?;
        let mut parser = Parser::new();
        parser.set_language(&language).ok()?;
        Some(parser)
    }

    /// Parse source code with a specific grammar.
    ///
    /// The grammar name should match tree-sitter grammar names (e.g., "python", "rust", "typescript").
    pub fn parse_with_grammar(&self, grammar: &str, source: &str) -> Option<tree_sitter::Tree> {
        let mut parser = self.parser_for(grammar)?;
        parser.parse(source, None)
    }

    /// List grammars available in external search paths.
    pub fn available_external_grammars(&self) -> Vec<String> {
        self.loader.available_external()
    }
}

impl Default for Parsers {
    fn default() -> Self {
        Self::new()
    }
}
