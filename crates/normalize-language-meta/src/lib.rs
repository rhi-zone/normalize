//! Language metadata and capabilities for normalize.
//!
//! This crate provides metadata about programming languages that is orthogonal
//! to syntax extraction (normalize-languages) and package discovery (normalize-local-deps).
//!
//! # Capabilities
//!
//! Languages have different capabilities that determine which analyses apply:
//!
//! ```
//! use normalize_language_meta::{capabilities_for, Capabilities};
//!
//! let rust_caps = capabilities_for("Rust");
//! assert!(rust_caps.imports);
//! assert!(rust_caps.callable_symbols);
//!
//! // Note: names must match Language::name() exactly (case-sensitive)
//! let json_caps = capabilities_for("JSON");
//! assert!(!json_caps.imports);  // JSON has no import system
//! ```
//!
//! # Future Metadata
//!
//! This crate is designed to grow with additional language metadata:
//! - Type system characteristics (static/dynamic, inference)
//! - Paradigm tags (functional, OOP, procedural)
//! - Syntax family (C-like, ML-like, Lisp-like)
//! - Common domains (web, systems, data science)

mod capabilities;
mod registry;

pub use capabilities::Capabilities;
pub use registry::{capabilities_for, register};
