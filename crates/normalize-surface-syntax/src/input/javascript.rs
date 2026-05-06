//! JavaScript reader — delegates to the TypeScript reader's grammar logic.
//!
//! The tree-sitter JavaScript grammar uses the same node kinds as the TypeScript
//! grammar for all constructs that exist in JavaScript (functions, classes, loops,
//! destructuring, template literals, async/await, etc.).  TypeScript-only node
//! kinds (`type_annotation`, `interface_declaration`, …) simply don't appear in
//! JavaScript sources, so the shared `ReadContext` logic handles them naturally by
//! skipping or ignoring them.

use crate::ir::*;
use crate::traits::{ReadError, Reader};

use super::typescript::read_with_language;

/// Static instance of the JavaScript reader for registry.
pub static JAVASCRIPT_READER: JavaScriptReader = JavaScriptReader;

/// JavaScript reader using tree-sitter.
pub struct JavaScriptReader;

impl Reader for JavaScriptReader {
    fn language(&self) -> &'static str {
        "javascript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn read(&self, source: &str) -> Result<Program, ReadError> {
        read_javascript(source)
    }
}

/// Parse JavaScript source into surface-syntax IR.
pub fn read_javascript(source: &str) -> Result<Program, ReadError> {
    read_with_language(source, arborium_javascript::language().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_let() -> Result<(), ReadError> {
        let program = read_javascript("let x = 42;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                name,
                init,
                mutable,
                ..
            } => {
                assert_eq!(name, "x");
                assert!(mutable);
                assert!(init.is_some());
            }
            _ => panic!("expected Let"),
        }
        Ok(())
    }

    #[test]
    fn test_class_declaration() -> Result<(), ReadError> {
        let program = read_javascript(
            "class Animal { constructor(name) { this.name = name; } speak() { return 1; } }",
        )?;
        assert!(!program.body.is_empty());
        Ok(())
    }

    #[test]
    fn test_template_literal() -> Result<(), ReadError> {
        let program = read_javascript("const msg = `Hello ${name}!`;")?;
        assert_eq!(program.body.len(), 1);
        Ok(())
    }

    #[test]
    fn test_async_await() -> Result<(), ReadError> {
        let program =
            read_javascript("async function f(url) { const r = await fetch(url); return r; }")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Function(f) => assert_eq!(f.name, "f"),
            _ => panic!("expected Function"),
        }
        Ok(())
    }

    #[test]
    fn test_destructuring() -> Result<(), ReadError> {
        let program = read_javascript("const { a, b } = obj;")?;
        assert!(!program.body.is_empty());
        Ok(())
    }

    #[test]
    fn test_rest_params() -> Result<(), ReadError> {
        let program = read_javascript("function sum(...args) { return args[0]; }")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Function(f) => {
                assert_eq!(f.params.len(), 1);
                assert_eq!(f.params[0].name, "args");
            }
            _ => panic!("expected Function"),
        }
        Ok(())
    }
}
