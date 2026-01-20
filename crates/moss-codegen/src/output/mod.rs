//! Output backends for code generation.
//!
//! Each backend takes an IR [`Schema`](crate::ir::Schema) and produces code.

#[cfg(feature = "typescript-types")]
pub mod typescript;

#[cfg(feature = "typescript-types")]
pub use typescript::{TypeScriptOptions, generate_typescript_types};

#[cfg(feature = "zod")]
pub mod zod;

#[cfg(feature = "zod")]
pub use zod::{ZodOptions, generate_zod};
