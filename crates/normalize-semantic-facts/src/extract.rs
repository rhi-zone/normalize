//! Generic extraction interface: a [`FactExtractor`] walks a tree-sitter CST
//! for one grammar and lowers the structural declarations it recognizes into
//! [`Fact`] occurrences.
//!
//! This prototype walks the CST directly in Rust rather than using `.scm`
//! query files (see `OVERVIEW.md` and the task that produced this crate —
//! query-based extraction is a follow-up once the IR shape has settled).

use crate::ir::Fact;

/// A fact together with where it was found. Locations are what the
/// restatement report groups by fact identity to show — see
/// `restatement.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FactOccurrence {
    pub fact: Fact,
    /// Source file this occurrence came from.
    pub file: String,
    /// 1-based line number of the source construct.
    pub line: usize,
}

/// Lowers a parsed CST for one language into fact IR occurrences.
///
/// Implementors are per-grammar (see `typescript.rs`, `sql.rs`). Each only
/// recognizes the structural constructs its language actually has — a
/// SQL extractor has no notion of a function declaration's parameter list,
/// for instance, so it simply never emits [`Fact::FunctionSignature`].
pub trait FactExtractor {
    /// The tree-sitter grammar name this extractor consumes (e.g.
    /// `"typescript"`, `"sql"`), matching [`normalize_languages::Language::grammar_name`].
    fn grammar_name(&self) -> &'static str;

    /// Walk `tree` (parsed from `source`) and emit every fact this extractor
    /// can recognize, tagged with `file` for location reporting.
    fn extract(&self, tree: &tree_sitter::Tree, source: &str, file: &str) -> Vec<FactOccurrence>;
}
