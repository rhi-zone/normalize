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
