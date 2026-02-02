//! ast-grep integration for pattern-based code search.
//!
//! This module provides an adapter that allows using ast-grep patterns
//! with our dynamically loaded tree-sitter grammars.

use ast_grep_core::matcher::PatternBuilder;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{Language as AstGrepLanguage, Pattern, PatternError};

// Re-export LanguageExt for convenience
pub use ast_grep_core::tree_sitter::LanguageExt as AstGrepLanguageExt;

/// A dynamically loaded language for ast-grep.
///
/// Wraps a tree-sitter Language to implement ast-grep's Language trait,
/// enabling pattern-based searches with dynamically loaded grammars.
#[derive(Clone)]
pub struct DynLang(pub tree_sitter::Language);

impl DynLang {
    /// Create a new DynLang from a tree-sitter Language.
    pub fn new(lang: tree_sitter::Language) -> Self {
        Self(lang)
    }

    /// Create a pattern from an ast-grep pattern string.
    pub fn pattern(&self, pattern: &str) -> Result<Pattern, PatternError> {
        Pattern::try_new(pattern, self.clone())
    }
}

impl AstGrepLanguage for DynLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.0.id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.0.field_id_for_name(field).map(|nz| nz.get())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
}

impl LanguageExt for DynLang {
    fn get_ts_language(&self) -> TSLanguage {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrammarLoader;

    #[test]
    fn test_pattern_matching() {
        use ast_grep_core::tree_sitter::LanguageExt;

        let loader = GrammarLoader::new();
        let Some(ts_lang) = loader.get("rust") else {
            eprintln!("Skipping test: rust grammar not available");
            return;
        };

        let lang = DynLang::new(ts_lang);
        let source = "fn foo() { let x = 1; }";

        let grep = lang.ast_grep(source);
        let root = grep.root();

        // Test pattern matching - find the identifier "foo"
        let pattern = lang.pattern("foo").expect("pattern failed");
        let matches: Vec<_> = root.find_all(&pattern).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text(), "foo");

        // Test pattern with metavariable - find let bindings
        let pattern = lang.pattern("let $X = $Y").expect("pattern failed");
        let matches: Vec<_> = root.find_all(&pattern).collect();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].text().contains("let x = 1"));
    }
}
