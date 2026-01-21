//! Polyglot type and validator generation from schemas.
//!
//! `moss-typegen` converts schema formats (JSON Schema, OpenAPI, Protobuf) into
//! idiomatic type definitions and runtime validators for multiple languages.
//!
//! # Architecture
//!
//! ```text
//! Input Formats          IR              Output Backends
//! ──────────────     ─────────────     ─────────────────
//! JSON Schema   ─┐                  ┌─> TypeScript types
//! OpenAPI       ─┼─> Schema ────────┼─> TypeScript validators (Zod, Valibot)
//! Protobuf      ─┘   (ir.rs)        ├─> Python types (dataclasses, TypedDict)
//!                                   ├─> Python validators (Pydantic)
//!                                   ├─> Go types (structs)
//!                                   └─> Rust types (serde structs)
//! ```
//!
//! # Example
//!
//! ```
//! use rhizome_moss_typegen::{input, output, ir::Schema};
//!
//! let json_schema = serde_json::json!({
//!     "type": "object",
//!     "title": "User",
//!     "properties": {
//!         "id": { "type": "string" },
//!         "name": { "type": "string" }
//!     },
//!     "required": ["id", "name"]
//! });
//!
//! // Parse JSON Schema to IR
//! let schema = input::parse_json_schema(&json_schema).unwrap();
//!
//! // Generate TypeScript
//! let ts = output::generate_typescript_types(&schema, &Default::default());
//! assert!(ts.contains("interface User"));
//! ```
//!
//! # Feature Flags
//!
//! Language umbrella flags (enable types + validators):
//! - `typescript` - TypeScript types + Zod + Valibot (default)
//! - `python` - Python types + Pydantic (default)
//! - `go` - Go structs (default)
//! - `rust-types` - Rust structs with serde (default)
//!
//! Per-language type flags:
//! - `typescript-types` - TypeScript interfaces/types
//! - `python-types` - Python dataclasses/TypedDict
//! - `go-types` - Go structs with json tags
//!
//! Validator flags:
//! - `zod` - Zod schema generation
//! - `valibot` - Valibot schema generation
//! - `pydantic` - Pydantic model generation

pub mod input;
pub mod ir;
pub mod output;

// Re-export commonly used items
pub use input::{ParseError, parse_json_schema, parse_openapi};

#[cfg(feature = "typescript-types")]
pub use output::generate_typescript_types;

#[cfg(feature = "zod")]
pub use output::generate_zod;

#[cfg(feature = "valibot")]
pub use output::generate_valibot;

#[cfg(feature = "python-types")]
pub use output::generate_python_types;

#[cfg(feature = "pydantic")]
pub use output::generate_pydantic;

#[cfg(feature = "go-types")]
pub use output::generate_go_types;

#[cfg(feature = "rust-types")]
pub use output::generate_rust_types;
