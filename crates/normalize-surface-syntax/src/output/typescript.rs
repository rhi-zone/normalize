//! TypeScript writer for surface-syntax IR.
//!
//! Emits surface-syntax IR as TypeScript source code.

use crate::ir::*;
use crate::traits::Writer;

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
                type_annotation,
                ..
            } => {
                self.output
                    .push_str(if *mutable { "let " } else { "const " });
                self.output.push_str(name);
                if let Some(t) = type_annotation {
                    self.output.push_str(": ");
                    self.output.push_str(t);
                }
                if let Some(init) = init {
                    self.output.push_str(" = ");
                    self.write_expr(init);
                }
                self.output.push(';');
            }

            Stmt::Destructure {
                pat,
                value,
                mutable,
                ..
            } => {
                self.output
                    .push_str(if *mutable { "let " } else { "const " });
                self.write_pat(pat);
                self.output.push_str(" = ");
                self.write_expr(value);
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
                ..
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

            Stmt::While { test, body, .. } => {
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
                ..
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
                ..
            } => {
                self.output.push_str("for (const ");
                self.output.push_str(variable);
                self.output.push_str(" of ");
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
                ..
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

            Stmt::Import { source, names, .. } => {
                self.output.push_str("import ");
                if names.is_empty() {
                    // Side-effect import: `import './side-effect'`
                    self.output.push('\'');
                    self.output.push_str(source);
                    self.output.push('\'');
                } else {
                    // Check if there is a namespace import
                    let namespace = names.iter().find(|n| n.is_namespace);
                    let default_name = names.iter().find(|n| !n.is_namespace && n.alias.is_none());
                    let named: Vec<_> = names.iter().filter(|n| !n.is_namespace).collect();

                    if let Some(ns) = namespace {
                        self.output.push_str("* as ");
                        self.output.push_str(ns.alias.as_deref().unwrap_or("_ns"));
                    } else if !named.is_empty() {
                        // Check for default import (single identifier before `{`)
                        // In our IR, default imports are ImportName { name, alias: None, is_namespace: false }
                        // and named imports can have aliases. We emit default first if present.
                        let has_default =
                            named.iter().any(|n| n.alias.is_none() && names.len() == 1);
                        let braced: Vec<_> = if has_default && named.len() == 1 {
                            // Only default import — no braces
                            let _ = default_name;
                            self.output.push_str(&named[0].name);
                            vec![]
                        } else {
                            named.clone()
                        };
                        if !braced.is_empty() {
                            self.output.push_str("{ ");
                            for (i, n) in braced.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(", ");
                                }
                                self.output.push_str(&n.name);
                                if let Some(alias) = &n.alias {
                                    self.output.push_str(" as ");
                                    self.output.push_str(alias);
                                }
                            }
                            self.output.push_str(" }");
                        }
                    }
                    self.output.push_str(" from '");
                    self.output.push_str(source);
                    self.output.push('\'');
                }
                self.output.push(';');
            }

            Stmt::Export { names, source, .. } => {
                self.output.push_str("export");
                if names.is_empty() {
                    // Empty export (e.g. `export default ...` stub)
                } else {
                    self.output.push_str(" { ");
                    for (i, n) in names.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.output.push_str(&n.name);
                        if let Some(alias) = &n.alias {
                            self.output.push_str(" as ");
                            self.output.push_str(alias);
                        }
                    }
                    self.output.push_str(" }");
                }
                if let Some(src) = source {
                    self.output.push_str(" from '");
                    self.output.push_str(src);
                    self.output.push('\'');
                }
                self.output.push(';');
            }

            Stmt::Class {
                name,
                extends,
                methods,
                ..
            } => {
                self.output.push_str("class ");
                self.output.push_str(name);
                if let Some(base) = extends {
                    self.output.push_str(" extends ");
                    self.output.push_str(base);
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                for method in methods {
                    self.write_indent();
                    if method.is_static {
                        self.output.push_str("static ");
                    }
                    self.output.push_str(&method.name);
                    self.output.push('(');
                    for (i, param) in method.params.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.output.push_str(&param.name);
                        if let Some(t) = &param.type_annotation {
                            self.output.push_str(": ");
                            self.output.push_str(t);
                        }
                    }
                    self.output.push(')');
                    if let Some(ret) = &method.return_type {
                        self.output.push_str(": ");
                        self.output.push_str(ret);
                    }
                    self.output.push_str(" {\n");
                    self.indent += 1;
                    for s in &method.body {
                        self.write_stmt(s);
                        self.output.push('\n');
                    }
                    self.indent -= 1;
                    self.write_indent();
                    self.output.push_str("}\n");
                }
                self.indent -= 1;
                self.write_indent();
                self.output.push('}');
            }

            Stmt::Comment { text, block, .. } => {
                if *block {
                    // Emit as JSDoc-style block comment if content has multiple lines
                    // or starts with `*`, otherwise as a simple `/* ... */`
                    if text.contains('\n') || text.starts_with('*') {
                        self.output.push_str("/**\n");
                        for line in text.lines() {
                            self.write_indent();
                            self.output.push_str(" * ");
                            self.output.push_str(line.trim_start_matches('*').trim());
                            self.output.push('\n');
                        }
                        self.write_indent();
                        self.output.push_str(" */");
                    } else {
                        self.output.push_str("/* ");
                        self.output.push_str(text);
                        self.output.push_str(" */");
                    }
                } else {
                    self.output.push_str("// ");
                    self.output.push_str(text);
                }
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
                type_annotation,
                ..
            } => {
                self.output
                    .push_str(if *mutable { "let " } else { "const " });
                self.output.push_str(name);
                if let Some(t) = type_annotation {
                    self.output.push_str(": ");
                    self.output.push_str(t);
                }
                if let Some(init) = init {
                    self.output.push_str(" = ");
                    self.write_expr(init);
                }
            }
            Stmt::Destructure {
                pat,
                value,
                mutable,
                ..
            } => {
                self.output
                    .push_str(if *mutable { "let " } else { "const " });
                self.write_pat(pat);
                self.output.push_str(" = ");
                self.write_expr(value);
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
            self.output.push_str("function ");
            self.output.push_str(&f.name);
            self.output.push('(');
        }
        for (i, param) in f.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&param.name);
            if let Some(t) = &param.type_annotation {
                self.output.push_str(": ");
                self.output.push_str(t);
            }
        }
        self.output.push(')');
        if let Some(ret) = &f.return_type {
            self.output.push_str(": ");
            self.output.push_str(ret);
        }
        self.output.push_str(" {\n");
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

            Expr::Binary {
                left, op, right, ..
            } => {
                self.output.push('(');
                self.write_expr(left);
                self.output.push(' ');
                self.write_binary_op(*op);
                self.output.push(' ');
                self.write_expr(right);
                self.output.push(')');
            }

            Expr::Unary { op, expr, .. } => {
                self.write_unary_op(*op);
                self.write_expr(expr);
            }

            Expr::Call { callee, args, .. } => {
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
                ..
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
                        self.output.push('"');
                        self.output.push_str(&escape_string(key));
                        self.output.push('"');
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
                        self.output.push_str(&param.name);
                        if let Some(t) = &param.type_annotation {
                            self.output.push_str(": ");
                            self.output.push_str(t);
                        }
                    }
                    self.output.push(')');
                    if let Some(ret) = &f.return_type {
                        self.output.push_str(": ");
                        self.output.push_str(ret);
                    }
                    self.output.push_str(" => ");

                    // Single return statement can be expression body
                    if f.body.len() == 1
                        && let Stmt::Return(Some(expr)) = &f.body[0]
                    {
                        self.write_expr(expr);
                        return;
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
                ..
            } => {
                self.output.push('(');
                self.write_expr(test);
                self.output.push_str(" ? ");
                self.write_expr(consequent);
                self.output.push_str(" : ");
                self.write_expr(alternate);
                self.output.push(')');
            }

            Expr::Assign { target, value, .. } => {
                self.write_expr(target);
                self.output.push_str(" = ");
                self.write_expr(value);
            }

            Expr::TemplateLiteral(parts) => {
                self.output.push('`');
                for part in parts {
                    match part {
                        TemplatePart::Text(s) => {
                            // Escape backticks and backslashes in template text
                            for ch in s.chars() {
                                match ch {
                                    '`' => self.output.push_str("\\`"),
                                    '\\' => self.output.push_str("\\\\"),
                                    '$' => self.output.push_str("\\$"),
                                    c => self.output.push(c),
                                }
                            }
                        }
                        TemplatePart::Expr(e) => {
                            self.output.push_str("${");
                            self.write_expr(e);
                            self.output.push('}');
                        }
                    }
                }
                self.output.push('`');
            }
        }
    }

    fn write_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Null => self.output.push_str("null"),
            Literal::Bool(b) => self.output.push_str(if *b { "true" } else { "false" }),
            Literal::Number(n) => {
                // Format number cleanly (no trailing .0 for integers)
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    self.output.push_str(&(*n as i64).to_string());
                } else {
                    self.output.push_str(&n.to_string());
                }
            }
            Literal::String(s) => {
                self.output.push('"');
                self.output.push_str(&escape_string(s));
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

    /// Write a binding pattern: `{ a, b: c }`, `[x, y]`, or plain `x`.
    fn write_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(name) => {
                self.output.push_str(name);
            }
            Pat::Object(fields) => {
                self.output.push_str("{ ");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    match &field.pat {
                        // `{ ...rest }` field — key is "..." sentinel
                        Pat::Rest(inner) => {
                            self.output.push_str("...");
                            self.write_pat(inner);
                        }
                        // shorthand `{ key }` when key == binding name
                        Pat::Ident(name) if name == &field.key => {
                            self.output.push_str(&field.key);
                        }
                        // renamed or nested `{ key: pat }`
                        _ => {
                            self.output.push_str(&field.key);
                            self.output.push_str(": ");
                            self.write_pat(&field.pat);
                        }
                    }
                    if let Some(default) = &field.default {
                        self.output.push_str(" = ");
                        self.write_expr(default);
                    }
                }
                self.output.push_str(" }");
            }
            Pat::Array(elements, rest) => {
                self.output.push('[');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    match elem {
                        None => {} // hole — just the comma is sufficient
                        Some(p) => self.write_pat(p),
                    }
                }
                if let Some(rest_name) = rest {
                    if !elements.is_empty() {
                        self.output.push_str(", ");
                    }
                    self.output.push_str("...");
                    self.output.push_str(rest_name);
                }
                self.output.push(']');
            }
            Pat::Rest(inner) => {
                self.output.push_str("...");
                self.write_pat(inner);
            }
        }
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
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_alphabetic() || first == '_' || first == '$')
                && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        }
    }
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
        use crate::Param;
        let program = Program::new(vec![Stmt::const_decl(
            "add",
            Expr::Function(Box::new(Function::anonymous(
                vec![Param::new("a"), Param::new("b")],
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

    #[test]
    fn test_line_comment() {
        let program = Program::new(vec![
            Stmt::comment_line("This is a line comment"),
            Stmt::const_decl("x", Expr::number(1)),
        ]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("// This is a line comment"));
        assert!(ts.contains("const x = 1;"));
    }

    #[test]
    fn test_block_comment() {
        let program = Program::new(vec![Stmt::comment_block("Block comment text")]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("/* Block comment text */"));
    }

    #[test]
    fn test_jsdoc_comment() {
        let program = Program::new(vec![Stmt::comment_block(
            "* Adds two numbers\n * @param a first\n * @param b second",
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("/**"));
        assert!(ts.contains(" * Adds two numbers"));
        assert!(ts.contains(" */"));
    }

    #[test]
    fn test_typed_function() {
        use crate::Param;
        let program = Program::new(vec![Stmt::function(crate::Function {
            name: "greet".to_string(),
            params: vec![
                Param::typed("name", "string"),
                Param::typed("age", "number"),
            ],
            return_type: Some("string".to_string()),
            body: vec![Stmt::return_stmt(Some(Expr::ident("name")))],
        })]);
        let ts = TypeScriptWriter::emit(&program);
        assert!(ts.contains("function greet(name: string, age: number): string {"));
    }

    #[test]
    fn test_typed_variable() {
        let program = Program::new(vec![Stmt::Let {
            name: "x".to_string(),
            init: Some(Expr::number(42)),
            mutable: false,
            type_annotation: Some("number".to_string()),
            span: None,
        }]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const x: number = 42;");
    }

    #[test]
    fn test_template_literal() {
        let program = Program::new(vec![Stmt::const_decl(
            "msg",
            Expr::TemplateLiteral(vec![
                TemplatePart::Text("Hello ".to_string()),
                TemplatePart::Expr(Box::new(Expr::ident("name"))),
                TemplatePart::Text("!".to_string()),
            ]),
        )]);
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), "const msg = `Hello ${name}!`;");
    }

    #[test]
    fn test_template_literal_round_trip() {
        use crate::input::read_typescript;
        let src = "const msg = `Hello ${name}!`;";
        let program = read_typescript(src).expect("parse failed");
        let ts = TypeScriptWriter::emit(&program);
        assert_eq!(ts.trim(), src);
    }
}
