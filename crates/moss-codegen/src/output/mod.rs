//! Output backends for code generation.
//!
//! Each backend takes an IR [`Schema`](crate::ir::Schema) and produces code.

// TypeScript
#[cfg(feature = "typescript-types")]
pub mod typescript;

#[cfg(feature = "typescript-types")]
pub use typescript::{OptionalStyle, TypeScriptOptions, generate_typescript_types};

// Zod (TypeScript validator)
#[cfg(feature = "zod")]
pub mod zod;

#[cfg(feature = "zod")]
pub use zod::{ZodOptions, generate_zod};

// Valibot (TypeScript validator)
#[cfg(feature = "valibot")]
pub mod valibot;

#[cfg(feature = "valibot")]
pub use valibot::{ValibotOptions, generate_valibot};

// Python
#[cfg(feature = "python-types")]
pub mod python;

#[cfg(feature = "python-types")]
pub use python::{PythonOptions, PythonStyle, generate_python_types};

// Pydantic (Python validator)
#[cfg(feature = "pydantic")]
pub mod pydantic;

#[cfg(feature = "pydantic")]
pub use pydantic::{PydanticOptions, PydanticVersion, generate_pydantic};

// Go
#[cfg(feature = "go-types")]
pub mod go;

#[cfg(feature = "go-types")]
pub use go::{GoOptions, generate_go_types};

// Rust
#[cfg(feature = "rust-types")]
pub mod rust;

#[cfg(feature = "rust-types")]
pub use rust::{RustOptions, generate_rust_types};
