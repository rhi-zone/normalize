//! Polyglot type and validator generation from schemas.
//!
//! `moss-codegen` converts schema formats (JSON Schema, OpenAPI, Protobuf) into
//! idiomatic type definitions and runtime validators for multiple languages.
//!
//! # Architecture
//!
//! ```text
//! Input Formats          IR              Output Backends
//! ──────────────     ─────────────     ─────────────────
//! JSON Schema   ─┐                  ┌─> TypeScript types
//! OpenAPI       ─┼─> Schema ────────┼─> TypeScript validators (Zod, etc.)
//! Protobuf      ─┘   (ir.rs)        ├─> Python types
//!                                   ├─> Python validators (Pydantic)
//!                                   ├─> Go types
//!                                   └─> Rust types
//! ```
//!
//! # Example
//!
//! ```
//! use rhizome_moss_codegen::{input, output, ir::Schema};
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
//! - `typescript` - TypeScript types + all validators (default)
//! - `typescript-types` - Just TypeScript interfaces
//! - `typescript-validators` - All TypeScript validators
//! - `zod` - Zod schema generation
//! - `valibot` - Valibot schema generation (TODO)
//! - `python` - Python types + validators (default)
//! - `python-types` - Just Python dataclasses/TypedDict
//! - `pydantic` - Pydantic model generation (TODO)
//! - `go` - Go struct generation (default)
//! - `rust-types` - Rust struct generation (default)

pub mod input;
pub mod ir;
pub mod output;

// Re-export commonly used items
pub use input::parse_json_schema;

#[cfg(feature = "typescript-types")]
pub use output::generate_typescript_types;

#[cfg(feature = "zod")]
pub use output::generate_zod;
