//! Python writer for surface-syntax IR.
//!
//! Emits surface-syntax IR as Python source code.

use crate::ir::*;
use crate::traits::Writer;
use std::fmt::Write;

/// Static instance of the Python writer for registry.
pub static PYTHON_WRITER: PythonWriterImpl = PythonWriterImpl;

/// Python writer implementing the Writer trait.
pub struct PythonWriterImpl;

impl Writer for PythonWriterImpl {
    fn language(&self) -> &'static str {
        "python"
    }

    fn extension(&self) -> &'static str {
        "py"
    }

    fn write(&self, program: &Program) -> String {
        PythonWriter::emit(program)
    }
}

/// Emits IR as Python source code.
pub struct PythonWriter {
    output: String,
    indent: usize,
}

impl PythonWriter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Emit a program to Python source.
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
            self.output.push_str("    ");
        }
    }

    fn write_stmt(&mut self, stmt: &Stmt) {
        self.write_indent();
        match stmt {
            Stmt::Expr(expr) => {
                self.write_expr(expr);
            }

            Stmt::Let { name, init, .. } => {
                // Python doesn't have variable declarations, just assignment
                self.output.push_str(name);
                if let Some(value) = init {
                    self.output.push_str(" = ");
                    self.write_expr(value);
                } else {
                    self.output.push_str(" = None");
                }
            }

            Stmt::Block(stmts) => {
                // Python doesn't have standalone blocks, emit as-is
                for s in stmts {
                    self.write_stmt(s);
                    self.output.push('\n');
                }
            }

            Stmt::If {
                test,
                consequent,
                alternate,
            } => {
                self.output.push_str("if ");
                self.write_expr(test);
                self.output.push_str(":\n");
                self.indent += 1;
                self.write_block_body(consequent);
                self.indent -= 1;

                if let Some(alt) = alternate {
                    self.write_indent();
                    // Check if alternate is another If (elif)
                    if let Stmt::If { .. } = alt.as_ref() {
                        self.output.push_str("el");
                        self.write_stmt_no_indent(alt);
                    } else {
                        self.output.push_str("else:\n");
                        self.indent += 1;
                        self.write_block_body(alt);
                        self.indent -= 1;
                    }
                }
            }

            Stmt::While { test, body } => {
                self.output.push_str("while ");
                self.write_expr(test);
                self.output.push_str(":\n");
                self.indent += 1;
                self.write_block_body(body);
                self.indent -= 1;
            }

            Stmt::For {
                init,
                test,
                update,
                body,
            } => {
                // C-style for loops don't exist in Python
                // Convert to while loop
                if let Some(i) = init {
                    self.write_stmt(i);
                    self.output.push('\n');
                    self.write_indent();
                }
                self.output.push_str("while ");
                if let Some(t) = test {
                    self.write_expr(t);
                } else {
                    self.output.push_str("True");
                }
                self.output.push_str(":\n");
                self.indent += 1;
                self.write_block_body(body);
                if let Some(u) = update {
                    self.write_indent();
                    self.write_expr(u);
                    self.output.push('\n');
                }
                self.indent -= 1;
            }

            Stmt::ForIn {
                variable,
                iterable,
                body,
            } => {
                self.output.push_str("for ");
                self.output.push_str(variable);
                self.output.push_str(" in ");
                self.write_expr(iterable);
                self.output.push_str(":\n");
                self.indent += 1;
                self.write_block_body(body);
                self.indent -= 1;
            }

            Stmt::Return(expr) => {
                self.output.push_str("return");
                if let Some(e) = expr {
                    self.output.push(' ');
                    self.write_expr(e);
                }
            }

            Stmt::Break => {
                self.output.push_str("break");
            }

            Stmt::Continue => {
                self.output.push_str("continue");
            }

            Stmt::Function(func) => {
                self.output.push_str("def ");
                if func.name.is_empty() {
                    self.output.push_str("_anonymous");
                } else {
                    self.output.push_str(&func.name);
                }
                self.output.push('(');
                self.output.push_str(&func.params.join(", "));
                self.output.push_str("):\n");
                self.indent += 1;
                if func.body.is_empty() {
                    self.write_indent();
                    self.output.push_str("pass");
                } else {
                    for s in &func.body {
                        self.write_stmt(s);
                        self.output.push('\n');
                    }
                }
                self.indent -= 1;
            }
        }
    }

    fn write_stmt_no_indent(&mut self, stmt: &Stmt) {
        // Write statement without leading indent (for elif)
        match stmt {
            Stmt::If {
                test,
                consequent,
                alternate,
            } => {
                self.output.push_str("if ");
                self.write_expr(test);
                self.output.push_str(":\n");
                self.indent += 1;
                self.write_block_body(consequent);
                self.indent -= 1;

                if let Some(alt) = alternate {
                    self.write_indent();
                    if let Stmt::If { .. } = alt.as_ref() {
                        self.output.push_str("el");
                        self.write_stmt_no_indent(alt);
                    } else {
                        self.output.push_str("else:\n");
                        self.indent += 1;
                        self.write_block_body(alt);
                        self.indent -= 1;
                    }
                }
            }
            _ => self.write_stmt(stmt),
        }
    }

    fn write_block_body(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(stmts) if stmts.is_empty() => {
                self.write_indent();
                self.output.push_str("pass\n");
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.write_stmt(s);
                    self.output.push('\n');
                }
            }
            _ => {
                self.write_stmt(stmt);
                self.output.push('\n');
            }
        }
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
                } else {
                    self.output.push('.');
                    // Extract property name from string literal
                    if let Expr::Literal(Literal::String(s)) = property.as_ref() {
                        self.output.push_str(s);
                    } else {
                        self.write_expr(property);
                    }
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
                self.output.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    // Python dict keys need quotes
                    let _ = write!(self.output, "\"{}\": ", key);
                    self.write_expr(value);
                }
                self.output.push('}');
            }

            Expr::Function(func) => {
                // Lambda if single return statement, otherwise can't express
                if func.body.len() == 1 {
                    if let Stmt::Return(Some(ret_expr)) = &func.body[0] {
                        self.output.push_str("lambda ");
                        self.output.push_str(&func.params.join(", "));
                        self.output.push_str(": ");
                        self.write_expr(ret_expr);
                        return;
                    }
                }
                // Can't express multi-statement function as expression in Python
                // Output as a comment or placeholder
                self.output.push_str("None  # complex function");
            }

            Expr::Conditional {
                test,
                consequent,
                alternate,
            } => {
                // Python ternary: consequent if test else alternate
                self.output.push('(');
                self.write_expr(consequent);
                self.output.push_str(" if ");
                self.write_expr(test);
                self.output.push_str(" else ");
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
            Literal::Null => self.output.push_str("None"),
            Literal::Bool(true) => self.output.push_str("True"),
            Literal::Bool(false) => self.output.push_str("False"),
            Literal::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    let _ = write!(self.output, "{}", *n as i64);
                } else {
                    let _ = write!(self.output, "{}", n);
                }
            }
            Literal::String(s) => {
                // Escape and quote
                self.output.push('"');
                for c in s.chars() {
                    match c {
                        '"' => self.output.push_str("\\\""),
                        '\\' => self.output.push_str("\\\\"),
                        '\n' => self.output.push_str("\\n"),
                        '\r' => self.output.push_str("\\r"),
                        '\t' => self.output.push_str("\\t"),
                        _ => self.output.push(c),
                    }
                }
                self.output.push('"');
            }
        }
    }

    fn write_binary_op(&mut self, op: BinaryOp) {
        let s = match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Concat => "+", // String concat in Python
        };
        self.output.push_str(s);
    }

    fn write_unary_op(&mut self, op: UnaryOp) {
        let s = match op {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "not ",
        };
        self.output.push_str(s);
    }
}

impl Default for PythonWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_assignment() {
        let program = Program {
            body: vec![Stmt::let_decl("x", Some(Expr::number(42)))],
        };
        let output = PythonWriter::emit(&program);
        assert_eq!(output.trim(), "x = 42");
    }

    #[test]
    fn test_function_call() {
        let program = Program {
            body: vec![Stmt::expr(Expr::call(
                Expr::ident("print"),
                vec![Expr::string("hello")],
            ))],
        };
        let output = PythonWriter::emit(&program);
        assert_eq!(output.trim(), "print(\"hello\")");
    }

    #[test]
    fn test_if_statement() {
        let program = Program {
            body: vec![Stmt::if_stmt(
                Expr::binary(Expr::ident("x"), BinaryOp::Gt, Expr::number(0)),
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("print"),
                    vec![Expr::ident("x")],
                ))]),
                None,
            )],
        };
        let output = PythonWriter::emit(&program);
        assert!(output.contains("if (x > 0):"));
        assert!(output.contains("print(x)"));
    }

    #[test]
    fn test_for_in_loop() {
        let program = Program {
            body: vec![Stmt::for_in(
                "item",
                Expr::ident("items"),
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("print"),
                    vec![Expr::ident("item")],
                ))]),
            )],
        };
        let output = PythonWriter::emit(&program);
        assert!(output.contains("for item in items:"));
    }

    #[test]
    fn test_function_definition() {
        let program = Program {
            body: vec![Stmt::function(Function::new(
                "add",
                vec!["a".into(), "b".into()],
                vec![Stmt::return_stmt(Some(Expr::binary(
                    Expr::ident("a"),
                    BinaryOp::Add,
                    Expr::ident("b"),
                )))],
            ))],
        };
        let output = PythonWriter::emit(&program);
        assert!(output.contains("def add(a, b):"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_list_literal() {
        let program = Program {
            body: vec![Stmt::let_decl(
                "arr",
                Some(Expr::array(vec![
                    Expr::number(1),
                    Expr::number(2),
                    Expr::number(3),
                ])),
            )],
        };
        let output = PythonWriter::emit(&program);
        assert_eq!(output.trim(), "arr = [1, 2, 3]");
    }

    #[test]
    fn test_dict_literal() {
        let program = Program {
            body: vec![Stmt::let_decl(
                "obj",
                Some(Expr::object(vec![
                    ("x".into(), Expr::number(1)),
                    ("y".into(), Expr::number(2)),
                ])),
            )],
        };
        let output = PythonWriter::emit(&program);
        assert!(output.contains("{\"x\": 1, \"y\": 2}"));
    }

    #[test]
    fn test_lambda() {
        let program = Program {
            body: vec![Stmt::let_decl(
                "add",
                Some(Expr::Function(Box::new(Function::anonymous(
                    vec!["a".into(), "b".into()],
                    vec![Stmt::return_stmt(Some(Expr::binary(
                        Expr::ident("a"),
                        BinaryOp::Add,
                        Expr::ident("b"),
                    )))],
                )))),
            )],
        };
        let output = PythonWriter::emit(&program);
        assert!(output.contains("lambda a, b: (a + b)"));
    }
}
