//! Core IR types for surface syntax translation.
//!
//! This IR is deliberately minimal and opcode-agnostic. It represents
//! common programming constructs without domain-specific knowledge.
//!
//! Domain operations like `lotus.spawn_entity(x)` are just function calls -
//! the runtime (spore) provides the actual implementations.

mod expr;
mod pat;
mod stmt;
mod structure_eq;

pub use expr::*;
pub use pat::*;
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

/// A function parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    /// Parameter name.
    pub name: String,
    /// Optional type annotation (e.g. `string` for `x: string` in TypeScript,
    /// `str` for `x: str` in Python). Carried as raw source text; not interpreted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<String>,
}

impl Param {
    /// Create a plain parameter with no type annotation.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_annotation: None,
        }
    }

    /// Create a typed parameter.
    pub fn typed(name: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_annotation: Some(annotation.into()),
        }
    }
}

impl From<&str> for Param {
    fn from(s: &str) -> Self {
        Param::new(s)
    }
}

impl From<String> for Param {
    fn from(s: String) -> Self {
        Param::new(s)
    }
}

/// A function definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    /// Function name (empty for anonymous functions).
    pub name: String,
    /// Parameters (with optional type annotations).
    pub params: Vec<Param>,
    /// Optional return type annotation (e.g. `string` for `): string` in TypeScript,
    /// `int` for `-> int` in Python).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    /// Function body.
    pub body: Vec<Stmt>,
}

impl Function {
    pub fn new(name: impl Into<String>, params: Vec<Param>, body: Vec<Stmt>) -> Self {
        Self {
            name: name.into(),
            params,
            return_type: None,
            body,
        }
    }

    pub fn anonymous(params: Vec<Param>, body: Vec<Stmt>) -> Self {
        Self::new("", params, body)
    }
}
