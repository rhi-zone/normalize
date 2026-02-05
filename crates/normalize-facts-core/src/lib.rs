//! Core data types for normalize facts.
//!
//! This crate defines the vocabulary for code facts - symbols, imports, exports,
//! and related metadata. These types are used by:
//! - `normalize-facts` for extraction and storage
//! - `normalize-facts-rules-api` for analysis rules
//! - `normalize-languages` for language-specific extraction

mod file;
mod import;
mod symbol;

pub use file::IndexedFile;
pub use import::{Export, FlatImport, Import};
pub use symbol::{FlatSymbol, Symbol, SymbolKind, Visibility, VisibilityMechanism};
