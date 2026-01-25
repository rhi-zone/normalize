//! Polyglot type and validator generation from schemas.
//!
//! `normalize-typegen` converts schema formats (JSON Schema, OpenAPI, Protobuf) into
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
//! use normalize_typegen::{input, output, ir::Schema};
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
//! # Using the Backend Registry
//!
//! ```ignore
//! use normalize_typegen::{get_backend, backend_names};
//!
//! // List all available backends
//! for name in backend_names() {
//!     println!("Backend: {}", name);
//! }
//!
//! // Get and use a specific backend
//! if let Some(backend) = get_backend("typescript") {
//!     let output = backend.generate(&schema);
//!     println!("{}", output);
//! }
//! ```
//!
//! # Feature Flags
//!
//! Backend flags (use `backend-*` prefix):
//! - `backend-typescript` - TypeScript interfaces/types
//! - `backend-zod` - Zod schema generation
//! - `backend-valibot` - Valibot schema generation
//! - `backend-python` - Python dataclasses/TypedDict
//! - `backend-pydantic` - Pydantic model generation
//! - `backend-go` - Go structs with json tags
//! - `backend-rust` - Rust structs with serde
//!
//! Language umbrella flags (convenience, enable types + validators):
//! - `typescript` - backend-typescript + backend-zod + backend-valibot
//! - `python` - backend-python + backend-pydantic
//! - `go` - backend-go
//! - `rust-types` - backend-rust

pub mod input;
pub mod ir;
pub mod output;
pub mod registry;
pub mod traits;

// Re-export commonly used items
#[cfg(feature = "input-typescript")]
pub use input::parse_typescript_types;
pub use input::{ParseError, parse_json_schema, parse_openapi};

// Re-export traits
pub use traits::{Backend, BackendCategory};

// Re-export registry functions
pub use registry::{
    backend_names, backends, backends_by_category, backends_for_language, get_backend,
    register_backend,
};

// Re-export generators (kept for backwards compatibility)
#[cfg(feature = "backend-typescript")]
pub use output::generate_typescript_types;

#[cfg(feature = "backend-zod")]
pub use output::generate_zod;

#[cfg(feature = "backend-valibot")]
pub use output::generate_valibot;

#[cfg(feature = "backend-python")]
pub use output::generate_python_types;

#[cfg(feature = "backend-pydantic")]
pub use output::generate_pydantic;

#[cfg(feature = "backend-go")]
pub use output::generate_go_types;

#[cfg(feature = "backend-rust")]
pub use output::generate_rust_types;

// Re-export backend structs
#[cfg(feature = "backend-typescript")]
pub use output::typescript::TypeScriptBackend;

#[cfg(feature = "backend-zod")]
pub use output::zod::ZodBackend;

#[cfg(feature = "backend-valibot")]
pub use output::valibot::ValibotBackend;

#[cfg(feature = "backend-python")]
pub use output::python::PythonBackend;

#[cfg(feature = "backend-pydantic")]
pub use output::pydantic::PydanticBackend;

#[cfg(feature = "backend-go")]
pub use output::go::GoBackend;

#[cfg(feature = "backend-rust")]
pub use output::rust::RustBackend;
