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
//! use rhizome_moss_surface_syntax::{input, output};
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

#[cfg(feature = "sexpr")]
pub mod sexpr;

pub mod input;
pub mod output;

// Re-exports
pub use ir::{BinaryOp, Expr, Function, Literal, Program, Stmt, UnaryOp};

#[cfg(feature = "read-typescript")]
pub use input::read_typescript;

#[cfg(feature = "write-lua")]
pub use output::LuaWriter;

#[cfg(feature = "sexpr")]
pub use sexpr::{SExpr, SExprError, from_sexpr, to_sexpr};
