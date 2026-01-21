//! Lua reader - parses Lua source into IR.
//!
//! TODO: Implement Lua parser using tree-sitter.

use crate::ir::Program;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("unsupported syntax: {0}")]
    Unsupported(String),
}

/// Parse Lua source into IR.
pub fn read_lua(_source: &str) -> Result<Program, ReadError> {
    Err(ReadError::Unsupported(
        "Lua reader not yet implemented".into(),
    ))
}
