//! JavaScript writer — delegates to the TypeScript writer.
//!
//! The surface-syntax IR emitted by the JavaScript reader is identical in
//! structure to the IR produced by the TypeScript reader.  The TypeScript
//! writer already emits valid JavaScript (it uses `===`/`!==`, arrow functions,
//! `let`/`const`, etc.), so JavaScript output is just TypeScript output with a
//! `.js` extension and "javascript" language label.

use crate::ir::Program;
use crate::output::typescript::TypeScriptWriter;
use crate::traits::Writer;

/// Static instance of the JavaScript writer for registry.
pub static JAVASCRIPT_WRITER: JavaScriptWriterImpl = JavaScriptWriterImpl;

/// JavaScript writer — delegates to the TypeScript writer.
pub struct JavaScriptWriterImpl;

impl Writer for JavaScriptWriterImpl {
    fn language(&self) -> &'static str {
        "javascript"
    }

    fn extension(&self) -> &'static str {
        "js"
    }

    fn write(&self, program: &Program) -> String {
        TypeScriptWriter::emit(program)
    }
}
