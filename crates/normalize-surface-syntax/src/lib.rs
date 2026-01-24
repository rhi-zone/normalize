//! Surface-level syntax translation between languages.
//!
//! `moss-surface-syntax` provides a common IR for imperative code and
//! translates between language syntaxes (TypeScript, Lua, etc.) at the
//! surface level - it maps syntax, not deep semantics.
//!
//! # Architecture
//!
//! ```text
//! Source Languages        IR              Target Languages
//! ────────────────    ─────────────    ────────────────────
//! TypeScript      ─┐                ┌─> TypeScript
//! Lua             ─┼─> Program ─────┼─> Lua
//! (future)        ─┘    (ir.rs)     └─> (future)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use normalize_surface_syntax::{input, output};
//!
//! // Read TypeScript
//! let ir = input::read_typescript("const x = 1 + 2;")?;
//!
//! // Write to Lua
//! let lua = output::LuaWriter::emit(&ir);
//! // => "local x = (1 + 2)"
//! ```
//!
//! # S-Expression Format
//!
//! The IR can be serialized to a compact S-expression format (JSON arrays):
//! - `["std.let", "x", 1]` → variable binding
//! - `["math.add", left, right]` → binary operation
//! - `["console.log", "hello"]` → function call
//!
//! This format is used for storage (e.g., lotus verbs).
//!
//! # Note on Translation Fidelity
//!
//! This is **surface-level** translation, not semantic transpilation like
//! Haxe or ReScript. The IR captures syntax structure; domain semantics
//! are handled by the runtime (e.g., spore).

pub mod ir;
pub mod registry;
pub mod traits;

#[cfg(feature = "sexpr")]
pub mod sexpr;

pub mod input;
pub mod output;

// Re-exports: IR types
pub use ir::{BinaryOp, Expr, Function, Literal, Program, Stmt, StructureEq, UnaryOp};

// Re-exports: Traits
pub use traits::{ReadError, Reader, Writer};

// Re-exports: Registry
pub use registry::{
    reader_for_extension, reader_for_language, readers, register_reader, register_writer,
    writer_for_language, writers,
};

// Re-exports: Built-in readers
#[cfg(feature = "read-typescript")]
pub use input::read_typescript;
#[cfg(feature = "read-typescript")]
pub use input::typescript::TypeScriptReader;

// Re-exports: Built-in writers
#[cfg(feature = "write-lua")]
pub use output::LuaWriter;
#[cfg(feature = "write-lua")]
pub use output::lua::LuaWriterImpl;

#[cfg(feature = "sexpr")]
pub use sexpr::{SExpr, SExprError, from_sexpr, to_sexpr};
