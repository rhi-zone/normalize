//! TypeScript writer for surface-syntax IR.
//!
//! Emits surface-syntax IR as TypeScript source code.

use crate::ir::*;
use crate::traits::Writer;
use std::fmt::Write;

/// Static instance of the TypeScript writer for registry.
pub static TYPESCRIPT_WRITER: TypeScriptWriterImpl = TypeScriptWriterImpl;

/// TypeScript writer implementing the Writer trait.
pub struct TypeScriptWriterImpl;

impl Writer for TypeScriptWriterImpl {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn extension(&self) -> &'static str {
        "ts"
    }

    fn write(&self, program: &Program) -> String {
        TypeScriptWriter::emit(program)
    }
}

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
    pub fn emit(program: &Program) -> String {
        let mut writer = Self::new();
        writer.write_program(program);
        writer.output
    }

    fn write_program(&mut self, program: &Program) {
        for stmt in &program.body {
            self.write_stmt(stmt);
            self.output.push('\n');
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }

    fn write_stmt(&mut self, stmt: &Stmt) {
        self.write_indent();
        match stmt {
            Stmt::Expr(expr) => {
                self.write_expr(expr);
                self.output.push(';');
            }

            Stmt::Let {
                name,
                init,
                mutable,
            } => {
                if *mutable {
                    write!(self.output, "let {}", name).unwrap();
                } else {
                    write!(self.output, "const {}", name).unwrap();
                }
                if let Some(init) = init {
                    self.output.push_str(" = ");
                    self.write_expr(init);
                }
                self.output.push(';');
            }

            Stmt::Block(stmts) => {
                self.output.push_str("{\n");
                self.indent += 1;
                for s in stmts {
                    self.write_stmt(s);
                    self.output.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }

            Stmt::If {
                test,
                consequent,
                alternate,
            } => {
                self.output.push_str("if (");
                self.write_expr(test);
                self.output.push_str(") ");
                self.write_block_stmt(consequent);
                if let Some(alt) = alternate {
                    self.output.push_str(" else ");
                    // Check if it's an else-if
                    if matches!(alt.as_ref(), Stmt::If { .. }) {
                        self.write_stmt_no_indent(alt);
                    } else {
                        self.write_block_stmt(alt);
                    }
                }
            }

            Stmt::While { test, body } => {
                self.output.push_str("while (");
                self.write_expr(test);
                self.output.push_str(") ");
                self.write_block_stmt(body);
            }

            Stmt::For {
                init,
                test,
                update,
                body,
            } => {
                self.output.push_str("for (");
                if let Some(init) = init {
                    self.write_stmt_inline(init);
                }
                self.output.push_str("; ");
                if let Some(test) = test {
                    self.write_expr(test);
                }
                self.output.push_str("; ");
                if let Some(update) = update {
                    self.write_expr(update);
                }
                self.output.push_str(") ");
                self.write_block_stmt(body);
            }

            Stmt::ForIn {
                variable,
                iterable,
                body,
            } => {
                write!(self.output, "for (const {} of ", variable).unwrap();
                self.write_expr(iterable);
                self.output.push_str(") ");
                self.write_block_stmt(body);
            }

            Stmt::Return(expr) => {
                self.output.push_str("return");
                if let Some(e) = expr {
                    self.output.push(' ');
                    self.write_expr(e);
                }
                self.output.push(';');
            }

            Stmt::Break => {
                self.output.push_str("break;");
            }

            Stmt::Continue => {
                self.output.push_str("continue;");
            }

            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                self.output.push_str("try ");
                self.write_block_stmt(body);
                if let Some(cb) = catch_body {
                    self.output.push_str(" catch");
                    if let Some(param) = catch_param {
                        self.output.push_str(" (");
                        self.output.push_str(param);
                        self.output.push(')');
                    }
                    self.output.push(' ');
                    self.write_block_stmt(cb);
                }
                if let Some(fb) = finally_body {
                    self.output.push_str(" finally ");
                    self.write_block_stmt(fb);
                }
            }

            Stmt::Function(f) => {
                self.write_function(f);
            }
        }
    }

    fn write_stmt_no_indent(&mut self, stmt: &Stmt) {
        // Write statement without the leading indent (for else-if chains)
        match stmt {
            Stmt::If { .. } => {
                // Save indent, set to 0, write, restore
                let saved_indent = self.indent;
                self.indent = 0;
                self.write_stmt(stmt);
                self.indent = saved_indent;
            }
            _ => self.write_stmt(stmt),
        }
    }

    fn write_stmt_inline(&mut self, stmt: &Stmt) {
        // Write statement without indent and without semicolon (for for-loop init)
        match stmt {
            Stmt::Let {
                name,
                init,
                mutable,
            } => {
                if *mutable {
                    write!(self.output, "let {}", name).unwrap();
                } else {
                    write!(self.output, "const {}", name).unwrap();
                }
                if let Some(init) = init {
                    self.output.push_str(" = ");
                    self.write_expr(init);
                }
            }
            Stmt::Expr(expr) => {
                self.write_expr(expr);
            }
            _ => {}
        }
    }

    fn write_block_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(stmts) => {
                self.output.push_str("{\n");
                self.indent += 1;
                for s in stmts {
                    self.write_stmt(s);
                    self.output.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }
            _ => {
                self.output.push_str("{\n");
                self.indent += 1;
                self.write_stmt(stmt);
                self.output.push('\n');
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }
        }
    }

    fn write_function(&mut self, f: &Function) {
        if f.name.is_empty() {
            self.output.push_str("function(");
        } else {
            write!(self.output, "function {}(", f.name).unwrap();
        }
        for (i, param) in f.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(param);
        }
        self.output.push_str(") {\n");
        self.indent += 1;
        for stmt in &f.body {
            self.write_stmt(stmt);
            self.output.push('\n');
        }
        self.indent -= 1;
        self.write_indent();
        self.output.push('}');
    }

    fn write_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(lit) => self.write_literal(lit),

            Expr::Ident(name) => {
                self.output.push_str(name);
            }

            Expr::Binary { left, op, right } => {
                self.output.push('(');
                self.write_expr(left);
                self.output.push(' ');
                self.write_binary_op(*op);
                self.output.push(' ');
                self.write_expr(right);
                self.output.push(')');
            }

            Expr::Unary { op, expr } => {
                self.write_unary_op(*op);
                self.write_expr(expr);
            }

            Expr::Call { callee, args } => {
                self.write_expr(callee);
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.write_expr(arg);
                }
                self.output.push(')');
            }

            Expr::Member {
                object,
                property,
                computed,
            } => {
                self.write_expr(object);
                if *computed {
                    self.output.push('[');
                    self.write_expr(property);
                    self.output.push(']');
                } else if let Expr::Literal(Literal::String(s)) = property.as_ref() {
                    self.output.push('.');
                    self.output.push_str(s);
                } else {
                    self.output.push('[');
                    self.write_expr(property);
                    self.output.push(']');
                }
            }

            Expr::Array(items) => {
                self.output.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.write_expr(item);
                }
                self.output.push(']');
            }

            Expr::Object(pairs) => {
                self.output.push_str("{ ");
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    // Check if key is a valid identifier
                    if is_valid_identifier(key) {
                        self.output.push_str(key);
                    } else {
                        write!(self.output, "\"{}\"", escape_string(key)).unwrap();
                    }
                    self.output.push_str(": ");
                    self.write_expr(value);
                }
                self.output.push_str(" }");
            }

            Expr::Function(f) => {
                // Use arrow function syntax for anonymous functions
                if f.name.is_empty() {
                    self.output.push('(');
                    for (i, param) in f.params.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.output.push_str(param);
                    }
                    self.output.push_str(") => ");

                    // Single return statement can be expression body
                    if f.body.len() == 1 {
                        if let Stmt::Return(Some(expr)) = &f.body[0] {
                            self.write_expr(expr);
                            return;
                        }
                    }

                    self.output.push_str("{\n");
                    self.indent += 1;
                    for stmt in &f.body {
                        self.write_stmt(stmt);
                        self.output.push('\n');
                    }
                    self.indent -= 1;
                    self.write_indent();
                    self.output.push('}');
                } else {
                    self.write_function(f);
                }
            }

            Expr::Conditional {
                test,
                consequent,
                alternate,
            } => {
                self.output.push('(');
                self.write_expr(test);
                self.output.push_str(" ? ");
                self.write_expr(consequent);
                self.output.push_str(" : ");
                self.write_expr(alternate);
                self.output.push(')');
            }

            Expr::Assign { target, value } => {
                self.write_expr(target);
                self.output.push_str(" = ");
                self.write_expr(value);
            }
        }
    }

    fn write_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Null => self.output.push_str("null"),
            Literal::Bool(b) => write!(self.output, "{}", b).unwrap(),
            Literal::Number(n) => {
                // Format number cleanly (no trailing .0 for integers)
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(self.output, "{}", *n as i64).unwrap();
                } else {
                    write!(self.output, "{}", n).unwrap();
                }
            }
            Literal::String(s) => write!(self.output, "\"{}\"", escape_string(s)).unwrap(),
        }
    }

    fn write_binary_op(&mut self, op: BinaryOp) {
        let s = match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "===",
            BinaryOp::Ne => "!==",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::Concat => "+", // TypeScript uses + for string concatenation
        };
        self.output.push_str(s);
    }

    fn write_unary_op(&mut self, op: UnaryOp) {
        let s = match op {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
        };
        self.output.push_str(s);
    }
}

impl Default for TypeScriptWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' && first != '$' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_const() {
        let program = Program::new(vec![Stmt::const_decl("x", Expr::number(42))]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const x = 42;");
    }

    #[test]
    fn test_simple_let() {
        let program = Program::new(vec![Stmt::let_decl("x", Some(Expr::number(42)))]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "let x = 42;");
    }

    #[test]
    fn test_function_call() {
        let program = Program::new(vec![Stmt::expr(Expr::call(
            Expr::member(Expr::ident("console"), "log"),
            vec![Expr::string("hello")],
        ))]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "console.log(\"hello\");");
    }

    #[test]
    fn test_binary_expr() {
        let program = Program::new(vec![Stmt::const_decl(
            "sum",
            Expr::binary(Expr::number(1), BinaryOp::Add, Expr::number(2)),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const sum = (1 + 2);");
    }

    #[test]
    fn test_arrow_function() {
        let program = Program::new(vec![Stmt::const_decl(
            "add",
            Expr::Function(Box::new(Function::anonymous(
                vec!["a".to_string(), "b".to_string()],
                vec![Stmt::return_stmt(Some(Expr::binary(
                    Expr::ident("a"),
                    BinaryOp::Add,
                    Expr::ident("b"),
                )))],
            ))),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const add = (a, b) => (a + b);");
    }

    #[test]
    fn test_if_statement() {
        let program = Program::new(vec![Stmt::if_stmt(
            Expr::binary(Expr::ident("x"), BinaryOp::Gt, Expr::number(0)),
            Stmt::return_stmt(Some(Expr::number(1))),
            Some(Stmt::return_stmt(Some(Expr::number(0)))),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("if ("));
        assert!(ts.contains("else"));
    }

    #[test]
    fn test_for_loop() {
        let program = Program::new(vec![Stmt::for_loop(
            Some(Stmt::let_decl("i", Some(Expr::number(0)))),
            Some(Expr::binary(
                Expr::ident("i"),
                BinaryOp::Lt,
                Expr::number(10),
            )),
            Some(Expr::assign(
                Expr::ident("i"),
                Expr::binary(Expr::ident("i"), BinaryOp::Add, Expr::number(1)),
            )),
            Stmt::block(vec![]),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("for (let i = 0; (i < 10); i = (i + 1))"));
    }

    #[test]
    fn test_object_literal() {
        let program = Program::new(vec![Stmt::const_decl(
            "obj",
            Expr::object(vec![
                ("a".to_string(), Expr::number(1)),
                ("b".to_string(), Expr::number(2)),
            ]),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const obj = { a: 1, b: 2 };");
    }
}
