//! Lua writer for surface-syntax IR.
//!
//! Emits surface-syntax IR as Lua source code.

use crate::ir::*;
use crate::traits::Writer;
use std::fmt::Write;

/// Static instance of the Lua writer for registry.
pub static LUA_WRITER: LuaWriterImpl = LuaWriterImpl;

/// Lua writer implementing the Writer trait.
pub struct LuaWriterImpl;

impl Writer for LuaWriterImpl {
    fn language(&self) -> &'static str {
        "lua"
    }

    fn extension(&self) -> &'static str {
        "lua"
    }

    fn write(&self, program: &Program) -> String {
        LuaWriter::emit(program)
    }
}

/// Emits IR as Lua source code.
pub struct LuaWriter {
    output: String,
    indent: usize,
}

impl LuaWriter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Emit a program to Lua source.
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
            }

            Stmt::Let { name, init, .. } => {
                write!(self.output, "local {}", name).unwrap();
                if let Some(init) = init {
                    self.output.push_str(" = ");
                    self.write_expr(init);
                }
            }

            Stmt::Block(stmts) => {
                self.output.push_str("do\n");
                self.indent += 1;
                for s in stmts {
                    self.write_stmt(s);
                    self.output.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("end");
            }

            Stmt::If {
                test,
                consequent,
                alternate,
            } => {
                self.output.push_str("if ");
                self.write_expr(test);
                self.output.push_str(" then\n");
                self.indent += 1;
                self.write_stmt_body(consequent);
                self.indent -= 1;
                if let Some(alt) = alternate {
                    self.write_indent();
                    self.output.push_str("else\n");
                    self.indent += 1;
                    self.write_stmt_body(alt);
                    self.indent -= 1;
                }
                self.write_indent();
                self.output.push_str("end");
            }

            Stmt::While { test, body } => {
                self.output.push_str("while ");
                self.write_expr(test);
                self.output.push_str(" do\n");
                self.indent += 1;
                self.write_stmt_body(body);
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("end");
            }

            Stmt::For {
                init,
                test,
                update,
                body,
            } => {
                // Lua doesn't have C-style for loops, emit as while
                if let Some(init) = init {
                    self.write_stmt(init);
                    self.output.push('\n');
                    self.write_indent();
                }
                self.output.push_str("while ");
                if let Some(test) = test {
                    self.write_expr(test);
                } else {
                    self.output.push_str("true");
                }
                self.output.push_str(" do\n");
                self.indent += 1;
                self.write_stmt_body(body);
                if let Some(update) = update {
                    self.write_indent();
                    self.write_expr(update);
                    self.output.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("end");
            }

            Stmt::ForIn {
                variable,
                iterable,
                body,
            } => {
                write!(self.output, "for _, {} in pairs(", variable).unwrap();
                self.write_expr(iterable);
                self.output.push_str(") do\n");
                self.indent += 1;
                self.write_stmt_body(body);
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("end");
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
                // Lua 5.1 doesn't have continue, use goto in 5.2+
                self.output
                    .push_str("-- continue (not supported in Lua 5.1)");
            }

            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                // Lua uses pcall/xpcall for error handling
                let param = catch_param.as_deref().unwrap_or("_err");
                self.output.push_str("local _ok, ");
                self.output.push_str(param);
                self.output.push_str(" = pcall(function()\n");
                self.indent += 1;
                self.write_stmt_body(body);
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("end)\n");
                if let Some(cb) = catch_body {
                    self.write_indent();
                    self.output.push_str("if not _ok then\n");
                    self.indent += 1;
                    self.write_stmt_body(cb);
                    self.indent -= 1;
                    self.write_indent();
                    self.output.push_str("end");
                }
                if let Some(fb) = finally_body {
                    self.output.push('\n');
                    self.write_stmt_body(fb);
                }
            }

            Stmt::Function(f) => {
                self.write_function(f);
            }
        }
    }

    fn write_stmt_body(&mut self, stmt: &Stmt) {
        match stmt {
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
        self.output.push_str(")\n");
        self.indent += 1;
        for stmt in &f.body {
            self.write_stmt(stmt);
            self.output.push('\n');
        }
        self.indent -= 1;
        self.write_indent();
        self.output.push_str("end");
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
                self.output.push('{');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.write_expr(item);
                }
                self.output.push('}');
            }

            Expr::Object(pairs) => {
                self.output.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    write!(self.output, "[\"{}\"] = ", key).unwrap();
                    self.write_expr(value);
                }
                self.output.push('}');
            }

            Expr::Function(f) => {
                self.write_function(f);
            }

            Expr::Conditional {
                test,
                consequent,
                alternate,
            } => {
                // Lua doesn't have ternary, use `a and b or c` pattern
                self.output.push('(');
                self.write_expr(test);
                self.output.push_str(" and ");
                self.write_expr(consequent);
                self.output.push_str(" or ");
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
            Literal::Null => self.output.push_str("nil"),
            Literal::Bool(b) => write!(self.output, "{}", b).unwrap(),
            Literal::Number(n) => write!(self.output, "{}", n).unwrap(),
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
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "~=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Concat => "..",
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

impl Default for LuaWriter {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_let() {
        let program = Program::new(vec![Stmt::const_decl("x", Expr::number(42))]);
        let lua = LuaWriter::emit(&program);
        assert_eq!(lua.trim(), "local x = 42");
    }

    #[test]
    fn test_function_call() {
        let program = Program::new(vec![Stmt::expr(Expr::call(
            Expr::member(Expr::ident("console"), "log"),
            vec![Expr::string("hello")],
        ))]);
        let lua = LuaWriter::emit(&program);
        assert_eq!(lua.trim(), "console.log(\"hello\")");
    }

    #[test]
    fn test_binary_expr() {
        let program = Program::new(vec![Stmt::const_decl(
            "sum",
            Expr::binary(Expr::number(1), BinaryOp::Add, Expr::number(2)),
        )]);
        let lua = LuaWriter::emit(&program);
        assert_eq!(lua.trim(), "local sum = (1 + 2)");
    }
}
