//! Output backends for code generation.
//!
//! Each backend takes an IR [`Schema`](crate::ir::Schema) and produces code.
//! All backends implement the [`Backend`](crate::traits::Backend) trait for
//! uniform access via the registry.

// TypeScript
#[cfg(feature = "backend-typescript")]
pub mod typescript;

#[cfg(feature = "backend-typescript")]
pub use typescript::{
    OptionalStyle, TypeScriptBackend, TypeScriptOptions, generate_typescript_types,
};

// Zod (TypeScript validator)
#[cfg(feature = "backend-zod")]
pub mod zod;

#[cfg(feature = "backend-zod")]
pub use zod::{ZodBackend, ZodOptions, generate_zod};

// Valibot (TypeScript validator)
#[cfg(feature = "backend-valibot")]
pub mod valibot;

#[cfg(feature = "backend-valibot")]
pub use valibot::{ValibotBackend, ValibotOptions, generate_valibot};

// Python
#[cfg(feature = "backend-python")]
pub mod python;

#[cfg(feature = "backend-python")]
pub use python::{PythonBackend, PythonOptions, PythonStyle, generate_python_types};

// Pydantic (Python validator)
#[cfg(feature = "backend-pydantic")]
pub mod pydantic;

#[cfg(feature = "backend-pydantic")]
pub use pydantic::{PydanticBackend, PydanticOptions, PydanticVersion, generate_pydantic};

// Go
#[cfg(feature = "backend-go")]
pub mod go;

#[cfg(feature = "backend-go")]
pub use go::{GoBackend, GoOptions, generate_go_types};

// Rust
#[cfg(feature = "backend-rust")]
pub mod rust;

#[cfg(feature = "backend-rust")]
pub use rust::{RustBackend, RustOptions, generate_rust_types};
