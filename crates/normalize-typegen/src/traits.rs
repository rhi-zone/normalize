//! Traits for code generation backends.

use crate::ir::Schema;

/// Category of backend output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendCategory {
    /// Pure type definitions (interfaces, classes, structs).
    Types,
    /// Runtime validators/schemas.
    Validators,
}

/// A code generation backend.
///
/// Backends transform an IR [`Schema`] into source code for a target language.
///
/// # Implementing Custom Backends
///
/// ```ignore
/// use normalize_typegen::{Backend, BackendCategory, ir::Schema, register_backend};
///
/// struct MyBackend;
///
/// impl Backend for MyBackend {
///     fn name(&self) -> &'static str { "my-backend" }
///     fn language(&self) -> &'static str { "kotlin" }
///     fn extension(&self) -> &'static str { "kt" }
///     fn category(&self) -> BackendCategory { BackendCategory::Types }
///     fn generate(&self, schema: &Schema) -> String { /* ... */ }
/// }
///
/// // Register before first use
/// register_backend(&MyBackend);
/// ```
pub trait Backend: Send + Sync {
    /// Unique backend identifier (e.g., "typescript", "zod", "pydantic").
    fn name(&self) -> &'static str;

    /// Target language (e.g., "typescript", "python", "rust").
    fn language(&self) -> &'static str;

    /// File extension for generated code (e.g., "ts", "py", "rs").
    fn extension(&self) -> &'static str;

    /// Category of output (types or validators).
    fn category(&self) -> BackendCategory;

    /// Generate code from the IR schema.
    fn generate(&self, schema: &Schema) -> String;
}
