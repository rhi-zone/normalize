//! Traits for language readers and writers.

use crate::ir::Program;

/// Error that can occur when reading source code into IR.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("unsupported syntax: {0}")]
    Unsupported(String),

    #[error("expected {expected}, got {got}")]
    UnexpectedNode { expected: String, got: String },
}

/// A reader parses source code into the surface-syntax IR.
pub trait Reader: Send + Sync {
    /// Language identifier (e.g., "typescript", "lua").
    fn language(&self) -> &'static str;

    /// File extensions this reader handles (e.g., &["ts", "tsx"]).
    fn extensions(&self) -> &'static [&'static str];

    /// Parse source code into the IR.
    fn read(&self, source: &str) -> Result<Program, ReadError>;
}

/// A writer emits the IR as source code in a target language.
pub trait Writer: Send + Sync {
    /// Language identifier (e.g., "lua", "typescript").
    fn language(&self) -> &'static str;

    /// File extension for output (e.g., "lua").
    fn extension(&self) -> &'static str;

    /// Emit the IR as source code.
    fn write(&self, program: &Program) -> String;
}
