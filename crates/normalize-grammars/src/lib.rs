//! Marker crate that pulls in all tree-sitter grammar dependencies for normalize.
//!
//! This crate has no code of its own — it exists solely to declare the grammar
//! crate dependencies so that they are compiled and linked into the binary.
//! Any binary that needs all supported grammars at runtime should depend on
//! this crate rather than listing each grammar crate individually.
