//! Generic extraction interface: a [`SemanticFactExtractor`] walks a
//! tree-sitter CST for one (or more) grammars and lowers the structural
//! declarations it recognizes into [`Fact`] occurrences.
//!
//! This prototype walks the CST directly in Rust rather than using `.scm`
//! query files (see `OVERVIEW.md` and the task that produced this crate —
//! query-based extraction is a follow-up once the IR shape has settled).
//!
//! Implementors are registered with the crate-wide registry (see
//! `registry.rs`) so callers look extractors up by grammar name via
//! [`crate::extractor_for_grammar`] / [`crate::extract_from_source`] rather
//! than constructing them directly.

use crate::ir::{Fact, NameConfig};

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

/// Lowers a parsed CST for one or more grammars into fact IR occurrences.
///
/// Implementors are per-language (see `typescript.rs`, `sql.rs`) and are
/// registered against every grammar name they handle (see
/// [`Self::grammar_names`]) — TypeScript's extractor, for instance, handles
/// both `"typescript"` and `"tsx"`, since the tsx grammar is a superset of
/// typescript's node types. Each extractor only recognizes the structural
/// constructs its language actually has — a SQL extractor has no notion of a
/// function declaration's parameter list, for instance, so it simply never
/// emits [`Fact::FunctionSignature`].
pub trait SemanticFactExtractor: Send + Sync {
    /// The tree-sitter grammar names this extractor consumes (e.g.
    /// `&["typescript", "tsx"]`, `&["sql"]`), matching
    /// [`normalize_languages::Language::grammar_name`]. The registry (see
    /// `registry.rs`) registers this extractor under every name returned
    /// here.
    fn grammar_names(&self) -> &'static [&'static str];

    /// Walk `tree` (parsed from `source`) and emit every fact this extractor
    /// can recognize, tagged with `file` for location reporting. `config`
    /// controls entity-name canonicalization (see [`NameConfig`]).
    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file: &str,
        config: &NameConfig,
    ) -> Vec<FactOccurrence>;
}
