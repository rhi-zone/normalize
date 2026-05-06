//! Core IR types for surface syntax translation.
//!
//! This IR is deliberately minimal and opcode-agnostic. It represents
//! common programming constructs without domain-specific knowledge.
//!
//! Domain operations like `lotus.spawn_entity(x)` are just function calls -
//! the runtime (spore) provides the actual implementations.

mod expr;
mod stmt;
mod structure_eq;

pub use expr::*;
pub use stmt::*;
pub use structure_eq::StructureEq;

use serde::{Deserialize, Serialize};

/// Source location span (1-based lines, 0-based columns).
///
/// Used for error messages ("expected foo at line 5:12") and debugging
/// round-trips. Writers ignore spans in their output — they are read-only
/// metadata populated by input readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    /// Create a span from tree-sitter `Point` values.
    ///
    /// Tree-sitter uses 0-based rows; we expose 1-based lines for
    /// human-readable error messages.
    pub fn from_ts(start: tree_sitter::Point, end: tree_sitter::Point) -> Self {
        Self {
            start_line: start.row as u32 + 1,
            start_col: start.column as u32,
            end_line: end.row as u32 + 1,
            end_col: end.column as u32,
        }
    }
}

/// A complete program/module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// Top-level statements.
    pub body: Vec<Stmt>,
}

impl Program {
    pub fn new(body: Vec<Stmt>) -> Self {
        Self { body }
    }
}

/// A function definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    /// Function name (empty for anonymous functions).
    pub name: String,
    /// Parameter names.
    pub params: Vec<String>,
    /// Function body.
    pub body: Vec<Stmt>,
}

impl Function {
    pub fn new(name: impl Into<String>, params: Vec<String>, body: Vec<Stmt>) -> Self {
        Self {
            name: name.into(),
            params,
            body,
        }
    }

    pub fn anonymous(params: Vec<String>, body: Vec<Stmt>) -> Self {
        Self::new("", params, body)
    }
}
