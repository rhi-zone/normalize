//! S-expression serialization for surface syntax IR.
//!
//! Converts between AST and S-expression format (JSON arrays).
//!
//! S-expression format: `["opcode", ...args]`
//! - `["std.let", "x", 1]` → variable binding
//! - `["math.add", left, right]` → binary operation
//! - `["std.if", cond, then, else]` → conditional
//!
//! This format is compact and used for storage (e.g., lotus verbs).

mod from_sexpr;
mod to_sexpr;

pub use from_sexpr::from_sexpr;
pub use to_sexpr::to_sexpr;

use serde_json::Value;
use thiserror::Error;

/// S-expression type alias (JSON Value).
pub type SExpr = Value;

#[derive(Debug, Error)]
pub enum SExprError {
    #[error("expected array, got {0}")]
    ExpectedArray(String),

    #[error("expected string opcode, got {0}")]
    ExpectedOpcode(String),

    #[error("unknown opcode: {0}")]
    UnknownOpcode(String),

    #[error("wrong number of arguments for {opcode}: expected {expected}, got {got}")]
    WrongArity {
        opcode: String,
        expected: usize,
        got: usize,
    },

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}
