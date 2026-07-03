//! Code fact extraction and storage library.
//!
//! This crate provides:
//! - Core fact types (symbols, imports, exports)
//! - Parser utilities for tree-sitter grammars
//! - Symbol extraction and flattening (`SymbolParser`)
//! - Fact storage (`FileIndex`)
//! - Trait definitions for fact extraction

mod ca_cache;
pub mod cfg_dataflow;
pub mod extract;
pub mod extraction_fixtures;
mod index;
mod parsers;
pub mod paths;
mod symbols;

#[cfg(feature = "cli")]
pub mod service;

pub use extract::{ExtractOptions, ExtractResult, Extractor, OnDemandResolver};
// InterfaceResolver moved to normalize-facts-core; re-export here for callers
pub use index::{CallGraphStats, ChangedFiles, FileIndex, IndexedFile, SymbolMatch};
pub use normalize_facts_core::InterfaceResolver;
pub use parsers::{
    MissingGrammar, available_external_grammars, grammar_loader, parse_with_grammar, parser_for,
    peek_missing_grammars, report_missing_grammar, take_missing_grammars, try_get_grammar,
};
pub use paths::get_normalize_dir;
pub use symbols::SymbolParser;

// Re-export core types for convenience
pub use normalize_facts_core::{
    Export, FlatImport, FlatSymbol, Import, Symbol, SymbolKind, Visibility,
};
