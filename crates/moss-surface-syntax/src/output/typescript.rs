//! TypeScript writer - emits IR as TypeScript source.
//!
//! TODO: Implement TypeScript emitter.

use crate::ir::Program;

/// Emits IR as TypeScript source code.
pub struct TypeScriptWriter {
    output: String,
    indent: usize,
}

impl TypeScriptWriter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Emit a program to TypeScript source.
    pub fn emit(_program: &Program) -> String {
        // TODO: Implement
        String::from("// TypeScript writer not yet implemented")
    }
}

impl Default for TypeScriptWriter {
    fn default() -> Self {
        Self::new()
    }
}
