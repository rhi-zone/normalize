//! Tree-sitter parser convenience re-exports.
//!
//! Delegates entirely to [`normalize_languages::parsers`] — the canonical
//! singleton lives there.  This module exists so that call sites inside
//! `normalize-facts` can write `crate::parsers::parse_with_grammar` without
//! importing from a sibling crate explicitly.
//!
//! # Lifetime safety
//!
//! The `GrammarLoader` is stored in a `'static OnceLock` inside
//! `normalize_languages::parsers`, so it outlives any `Tree` produced here.
//! Trees must be dropped before the end of the extraction call that created
//! them; they must not be stored in long-lived structs.

use std::sync::Arc;

pub use normalize_languages::parsers::{
    available_external_grammars, parse_with_grammar, parser_for,
};

/// Get the global grammar loader singleton (canonical instance from `normalize-languages`).
pub fn grammar_loader() -> Arc<normalize_languages::GrammarLoader> {
    normalize_languages::parsers::grammar_loader()
}
