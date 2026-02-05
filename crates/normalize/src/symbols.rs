//! Symbol parsing for indexing.
//!
//! This module re-exports SymbolParser from normalize_facts and provides
//! convenient re-exports of related types.

// Re-export SymbolParser from normalize-facts
pub use normalize_facts::SymbolParser;

// Re-export core types for backwards compatibility
pub use normalize_facts_core::{FlatImport, FlatSymbol, SymbolKind};
