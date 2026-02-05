//! Code fact extraction and storage library.
//!
//! This crate provides:
//! - Core fact types (symbols, imports, exports)
//! - Parser utilities for tree-sitter grammars
//! - Symbol extraction and flattening (`SymbolParser`)
//! - Fact storage (`FileIndex`)
//! - Trait definitions for fact extraction

pub mod extract;
mod index;
mod parsers;
mod symbols;

pub use extract::{ExtractOptions, ExtractResult, Extractor, InterfaceResolver, OnDemandResolver};
pub use index::{CallGraphStats, ChangedFiles, FileIndex, IndexedFile, SymbolMatch};
pub use parsers::{available_external_grammars, grammar_loader, parse_with_grammar, parser_for};
pub use symbols::SymbolParser;

// Re-export core types for convenience
pub use normalize_facts_core::{
    Export, FlatImport, FlatSymbol, Import, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
