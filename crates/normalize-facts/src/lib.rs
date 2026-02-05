//! Code fact extraction and storage library.
//!
//! This crate provides:
//! - Core fact types (symbols, imports, exports)
//! - Parser utilities for tree-sitter grammars
//! - Trait definitions for fact extraction
//!
//! The full `FileIndex` storage implementation is currently in the `normalize` crate.
//! This will be migrated here as part of the facts & rules architecture.

pub mod extract;
mod parsers;

pub use extract::{ExtractOptions, ExtractResult, Extractor, InterfaceResolver, OnDemandResolver};
pub use parsers::{available_external_grammars, grammar_loader, parse_with_grammar, parser_for};

// Re-export core types for convenience
pub use normalize_facts_core::{
    Export, FlatImport, FlatSymbol, Import, IndexedFile, Symbol, SymbolKind, Visibility,
    VisibilityMechanism,
};
