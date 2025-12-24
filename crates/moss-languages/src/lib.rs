//! Language support for moss.
//!
//! This crate provides the `LanguageSupport` trait and implementations for
//! various programming languages. Each language struct IS its support implementation.
//!
//! # Features
//!
//! - `all-languages` (default): Enable all supported languages
//! - `tier1`: Enable most common languages (Python, Rust, JS, TS, Go, Java, C++)
//! - `lang-python`, `lang-rust`, etc.: Enable individual languages
//!
//! # Example
//!
//! ```ignore
//! use moss_languages::{Python, LanguageSupport, support_for_path};
//! use std::path::Path;
//!
//! // Static usage (compile-time known language):
//! println!("Python function kinds: {:?}", Python.function_kinds());
//!
//! // Dynamic lookup (from file path):
//! if let Some(support) = support_for_path(Path::new("foo.py")) {
//!     println!("Language: {}", support.name());
//! }
//! ```

mod registry;
mod traits;
pub mod ecmascript;
#[cfg(any(feature = "lang-c", feature = "lang-cpp"))]
pub mod c_cpp;
pub mod external_packages;

// Language implementations
#[cfg(feature = "lang-python")]
pub mod python;

#[cfg(feature = "lang-rust")]
pub mod rust;

#[cfg(feature = "lang-javascript")]
pub mod javascript;

#[cfg(feature = "lang-typescript")]
pub mod typescript;

#[cfg(feature = "lang-go")]
pub mod go;

#[cfg(feature = "lang-java")]
pub mod java;

#[cfg(feature = "lang-c")]
pub mod c;

#[cfg(feature = "lang-cpp")]
pub mod cpp;

#[cfg(feature = "lang-ruby")]
pub mod ruby;

#[cfg(feature = "lang-scala")]
pub mod scala;

#[cfg(feature = "lang-vue")]
pub mod vue;

#[cfg(feature = "lang-markdown")]
pub mod markdown;

#[cfg(feature = "lang-json")]
pub mod json;

#[cfg(feature = "lang-yaml")]
pub mod yaml;

#[cfg(feature = "lang-toml")]
pub mod toml;

#[cfg(feature = "lang-html")]
pub mod html;

#[cfg(feature = "lang-css")]
pub mod css;

#[cfg(feature = "lang-bash")]
pub mod bash;

// Re-exports from registry
pub use registry::{support_for_extension, support_for_path, supported_languages};

// Re-exports from traits
pub use traits::{
    Export, Import, LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};

// Re-export language structs
#[cfg(feature = "lang-python")]
pub use python::Python;

#[cfg(feature = "lang-rust")]
pub use rust::Rust;

#[cfg(feature = "lang-javascript")]
pub use javascript::JavaScript;

#[cfg(feature = "lang-typescript")]
pub use typescript::{TypeScript, Tsx};

#[cfg(feature = "lang-go")]
pub use go::Go;

#[cfg(feature = "lang-java")]
pub use java::Java;

#[cfg(feature = "lang-c")]
pub use c::C;

#[cfg(feature = "lang-cpp")]
pub use cpp::Cpp;

#[cfg(feature = "lang-ruby")]
pub use ruby::Ruby;

#[cfg(feature = "lang-scala")]
pub use scala::Scala;

#[cfg(feature = "lang-vue")]
pub use vue::Vue;

#[cfg(feature = "lang-markdown")]
pub use markdown::Markdown;

#[cfg(feature = "lang-json")]
pub use json::Json;

#[cfg(feature = "lang-yaml")]
pub use yaml::Yaml;

#[cfg(feature = "lang-toml")]
pub use toml::Toml;

#[cfg(feature = "lang-html")]
pub use html::Html;

#[cfg(feature = "lang-css")]
pub use css::Css;

#[cfg(feature = "lang-bash")]
pub use bash::Bash;
