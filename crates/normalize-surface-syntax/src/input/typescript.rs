//! Tree-sitter based TypeScript reader.

use crate::ir::*;
use crate::traits::{ReadError, Reader};
use tree_sitter::{Node, Parser, Tree};

/// Static instance of the TypeScript reader for registry.
pub static TYPESCRIPT_READER: TypeScriptReader = TypeScriptReader;

/// TypeScript/TSX reader using tree-sitter.
pub struct TypeScriptReader;

impl Reader for TypeScriptReader {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts", "cts"]
    }

    fn read(&self, source: &str) -> Result<Program, ReadError> {
        read_typescript(source)
    }
}

/// Parse TypeScript source into surface-syntax IR.
pub fn read_typescript(source: &str) -> Result<Program, ReadError> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_typescript::language().into())
        .map_err(|err| ReadError::Parse(err.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ReadError::Parse("failed to parse".into()))?;

    let ctx = ReadContext::new(source);
    ctx.read_program(&tree)
}

struct ReadContext<'a> {
    source: &'a str,
}

impl<'a> ReadContext<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    fn read_program(&self, tree: &Tree) -> Result<Program, ReadError> {
        let root = tree.root_node();

        if root.has_error() {
            return Err(ReadError::Parse("syntax error in source".into()));
        }

        let mut statements = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.is_named() {
                if let Some(stmt) = self.read_stmt(child)? {
                    statements.push(stmt);
                }
            }
        }

        Ok(Program::new(statements))
    }

    fn read_stmt(&self, node: Node) -> Result<Option<Stmt>, ReadError> {
        match node.kind() {
            // Comments and empty statements (skip)
            "comment" | "empty_statement" => Ok(None),

            // Statements
            "expression_statement" => self.read_expression_statement(node).map(Some),
            "lexical_declaration" => self.read_lexical_declaration(node).map(Some),
            "variable_declaration" => self.read_variable_declaration(node).map(Some),
            "if_statement" => self.read_if_statement(node).map(Some),
            "while_statement" => self.read_while_statement(node).map(Some),
            "for_statement" => self.read_for_statement(node).map(Some),
            "for_in_statement" => self.read_for_in_statement(node).map(Some),
            "switch_statement" => self.read_switch_statement(node).map(Some),
            "try_statement" => self.read_try_statement(node).map(Some),
            "break_statement" => Ok(Some(Stmt::break_stmt())),
            "continue_statement" => Ok(Some(Stmt::continue_stmt())),
            "return_statement" => self.read_return_statement(node).map(Some),
            "statement_block" => self.read_block(node).map(Some),
            "function_declaration" => self.read_function_declaration(node).map(Some),

            // else_clause: extract the body
            "else_clause" => self.read_else_clause(node).map(Some),

            // Expressions become expression statements
            _ => {
                let expr = self.read_expr(node)?;
                Ok(Some(Stmt::expr(expr)))
            }
        }
    }

    fn read_expr(&self, node: Node) -> Result<Expr, ReadError> {
        match node.kind() {
            // Literals
            "number" => self.read_number(node),
            "string" => self.read_string(node),
            "true" => Ok(Expr::bool(true)),
            "false" => Ok(Expr::bool(false)),
            "null" | "undefined" => Ok(Expr::null()),

            // Expressions
            "identifier" => Ok(Expr::ident(self.node_text(node))),
            "this" => Ok(Expr::ident("this")),
            "binary_expression" => self.read_binary_expr(node),
            "unary_expression" => self.read_unary_expr(node),
            "parenthesized_expression" => self.read_parenthesized(node),
            "assignment_expression" => self.read_assignment_expr(node),
            "augmented_assignment_expression" => self.read_augmented_assignment_expr(node),
            "call_expression" => self.read_call_expr(node),
            "member_expression" => self.read_member_expr(node),
            "subscript_expression" => self.read_subscript_expr(node),
            "array" => self.read_array(node),
            "object" => self.read_object(node),
            "template_string" => self.read_template_string(node),
            "arrow_function" => self.read_arrow_function(node),
            "function" => self.read_function_expr(node),
            "ternary_expression" => self.read_ternary(node),

            // Type assertions - just pass through the inner expression
            "as_expression" => self.read_as_expression(node),
            "non_null_expression" => self.read_non_null_expression(node),

            kind => Err(ReadError::Unsupported(format!(
                "expression type '{}': {}",
                kind,
                self.node_text(node)
            ))),
        }
    }

    fn read_number(&self, node: Node) -> Result<Expr, ReadError> {
        let text = self.node_text(node);
        // Strip numeric separators (e.g., 10_000 -> 10000)
        let clean_text = text.replace('_', "");
        let value: f64 = clean_text
            .parse()
            .map_err(|_| ReadError::Parse(format!("invalid number: {}", text)))?;
        Ok(Expr::number(value))
    }

    fn read_string(&self, node: Node) -> Result<Expr, ReadError> {
        let text = self.node_text(node);
        // Remove quotes and handle escapes
        let inner = if text.starts_with('"') || text.starts_with('\'') {
            &text[1..text.len() - 1]
        } else if text.starts_with('`') {
            // Template literal - basic support
            &text[1..text.len() - 1]
        } else {
            text
        };
        // TODO: proper escape handling
        let unescaped = inner
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\'", "'")
            .replace("\\\\", "\\");
        Ok(Expr::string(unescaped))
    }

    /// Handle template strings with interpolation (e.g., `Hello ${name}!`)
    fn read_template_string(&self, node: Node) -> Result<Expr, ReadError> {
        let mut parts: Vec<Expr> = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                // String fragment between interpolations
                "string_fragment" | "template_fragment" => {
                    let text = self.node_text(child);
                    if !text.is_empty() {
                        parts.push(Expr::string(text));
                    }
                }
                // Interpolation: ${...}
                "template_substitution" => {
                    // Find the expression inside the ${ }
                    if let Some(expr) = child.named_child(0) {
                        parts.push(self.read_expr(expr)?);
                    }
                }
                // Skip the ` characters
                "`" => {}
                _ => {}
            }
        }

        // If no parts, return empty string
        if parts.is_empty() {
            return Ok(Expr::string(""));
        }

        // If single part that's a string, return it directly
        if parts.len() == 1 {
            return Ok(parts.remove(0));
        }

        // Multiple parts: chain concat operations
        let mut result = parts.remove(0);
        for part in parts {
            result = Expr::binary(result, BinaryOp::Concat, part);
        }
        Ok(result)
    }

    /// Handle TypeScript `as` type assertions
    fn read_as_expression(&self, node: Node) -> Result<Expr, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named()
                && child.kind() != "type_identifier"
                && !child.kind().contains("type")
            {
                return self.read_expr(child);
            }
        }
        let expr = node
            .named_child(0)
            .ok_or_else(|| ReadError::Parse("as_expression missing expression".into()))?;
        self.read_expr(expr)
    }

    /// Handle TypeScript non-null assertions (e.g., `foo!`)
    fn read_non_null_expression(&self, node: Node) -> Result<Expr, ReadError> {
        let expr = node
            .named_child(0)
            .ok_or_else(|| ReadError::Parse("non_null_expression missing expression".into()))?;
        self.read_expr(expr)
    }

    fn read_binary_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("binary_expression missing left".into()))?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("binary_expression missing right".into()))?;
        let operator = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("binary_expression missing operator".into()))?;

        let left_expr = self.read_expr(left)?;
        let right_expr = self.read_expr(right)?;
        let op_text = self.node_text(operator);

        let op = match op_text {
            // Arithmetic
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            "%" => BinaryOp::Mod,

            // Comparison
            "==" | "===" => BinaryOp::Eq,
            "!=" | "!==" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            ">" => BinaryOp::Gt,
            "<=" => BinaryOp::Le,
            ">=" => BinaryOp::Ge,

            // Logical
            "&&" => BinaryOp::And,
            "||" => BinaryOp::Or,

            // Operators that don't map directly - emit as function call
            "**" => {
                return Ok(Expr::call(
                    Expr::member(Expr::ident("math"), "pow"),
                    vec![left_expr, right_expr],
                ));
            }
            "??" => {
                return Ok(Expr::call(
                    Expr::member(Expr::ident("bool"), "nullish"),
                    vec![left_expr, right_expr],
                ));
            }

            _ => {
                return Err(ReadError::Unsupported(format!("operator '{}'", op_text)));
            }
        };

        Ok(Expr::binary(left_expr, op, right_expr))
    }

    fn read_unary_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let operator = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("unary_expression missing operator".into()))?;
        let argument = node
            .child_by_field_name("argument")
            .ok_or_else(|| ReadError::Parse("unary_expression missing argument".into()))?;

        let arg_expr = self.read_expr(argument)?;
        let op_text = self.node_text(operator);

        let op = match op_text {
            "!" => UnaryOp::Not,
            "-" => UnaryOp::Neg,
            "+" => return Ok(arg_expr), // Unary + is a no-op
            _ => {
                return Err(ReadError::Unsupported(format!(
                    "unary operator '{}'",
                    op_text
                )));
            }
        };

        Ok(Expr::unary(op, arg_expr))
    }

    fn read_parenthesized(&self, node: Node) -> Result<Expr, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                return self.read_expr(child);
            }
        }
        Err(ReadError::Parse("empty parenthesized expression".into()))
    }

    fn read_assignment_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("assignment missing left".into()))?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("assignment missing right".into()))?;

        let right_expr = self.read_expr(right)?;
        let left_expr = self.read_expr(left)?;

        Ok(Expr::assign(left_expr, right_expr))
    }

    fn read_augmented_assignment_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("augmented assignment missing left".into()))?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("augmented assignment missing right".into()))?;
        let operator = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("augmented assignment missing operator".into()))?;

        let left_expr = self.read_expr(left)?;
        let right_expr = self.read_expr(right)?;
        let op_text = self.node_text(operator);

        // Get the operation (strip the '=' suffix)
        let op = match op_text {
            "+=" => BinaryOp::Add,
            "-=" => BinaryOp::Sub,
            "*=" => BinaryOp::Mul,
            "/=" => BinaryOp::Div,
            "%=" => BinaryOp::Mod,
            "&&=" => BinaryOp::And,
            "||=" => BinaryOp::Or,
            "**=" => {
                // x **= y -> x = math.pow(x, y)
                let pow_call = Expr::call(
                    Expr::member(Expr::ident("math"), "pow"),
                    vec![left_expr.clone(), right_expr],
                );
                return Ok(Expr::assign(left_expr, pow_call));
            }
            "??=" => {
                // x ??= y -> x = bool.nullish(x, y)
                let nullish_call = Expr::call(
                    Expr::member(Expr::ident("bool"), "nullish"),
                    vec![left_expr.clone(), right_expr],
                );
                return Ok(Expr::assign(left_expr, nullish_call));
            }
            _ => {
                return Err(ReadError::Unsupported(format!(
                    "augmented assignment operator '{}'",
                    op_text
                )));
            }
        };

        // Build: left = left op right
        let operation = Expr::binary(left_expr.clone(), op, right_expr);
        Ok(Expr::assign(left_expr, operation))
    }

    fn read_call_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let function = node
            .child_by_field_name("function")
            .ok_or_else(|| ReadError::Parse("call_expression missing function".into()))?;
        let arguments = node
            .child_by_field_name("arguments")
            .ok_or_else(|| ReadError::Parse("call_expression missing arguments".into()))?;

        // Parse arguments
        let mut args = Vec::new();
        let mut cursor = arguments.walk();
        for child in arguments.children(&mut cursor) {
            if child.is_named() {
                args.push(self.read_expr(child)?);
            }
        }

        let callee = self.read_expr(function)?;
        Ok(Expr::call(callee, args))
    }

    fn read_member_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let object = node
            .child_by_field_name("object")
            .ok_or_else(|| ReadError::Parse("member_expression missing object".into()))?;
        let property = node
            .child_by_field_name("property")
            .ok_or_else(|| ReadError::Parse("member_expression missing property".into()))?;

        let obj_expr = self.read_expr(object)?;
        let prop_name = self.node_text(property);

        Ok(Expr::member(obj_expr, prop_name))
    }

    fn read_subscript_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let object = node
            .child_by_field_name("object")
            .ok_or_else(|| ReadError::Parse("subscript_expression missing object".into()))?;
        let index = node
            .child_by_field_name("index")
            .ok_or_else(|| ReadError::Parse("subscript_expression missing index".into()))?;

        let obj_expr = self.read_expr(object)?;
        let idx_expr = self.read_expr(index)?;

        Ok(Expr::index(obj_expr, idx_expr))
    }

    fn read_array(&self, node: Node) -> Result<Expr, ReadError> {
        let mut elements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                elements.push(self.read_expr(child)?);
            }
        }

        Ok(Expr::array(elements))
    }

    fn read_object(&self, node: Node) -> Result<Expr, ReadError> {
        let mut pairs = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "pair" {
                let key = child
                    .child_by_field_name("key")
                    .ok_or_else(|| ReadError::Parse("pair missing key".into()))?;
                let value = child
                    .child_by_field_name("value")
                    .ok_or_else(|| ReadError::Parse("pair missing value".into()))?;

                let key_str = match key.kind() {
                    "property_identifier" | "identifier" => self.node_text(key).to_string(),
                    "string" => {
                        let text = self.node_text(key);
                        text[1..text.len() - 1].to_string()
                    }
                    "number" => self.node_text(key).to_string(),
                    "computed_property_name" => {
                        // Computed property: [expr]: value - not directly supported in IR
                        // Fall back to using the expression text as key
                        let inner = key
                            .named_child(0)
                            .ok_or_else(|| ReadError::Parse("empty computed property".into()))?;
                        self.node_text(inner).to_string()
                    }
                    _ => {
                        return Err(ReadError::Unsupported(format!(
                            "object key type '{}'",
                            key.kind()
                        )));
                    }
                };

                pairs.push((key_str, self.read_expr(value)?));
            } else if child.kind() == "shorthand_property_identifier" {
                // { foo } is shorthand for { foo: foo }
                let name = self.node_text(child).to_string();
                pairs.push((name.clone(), Expr::ident(name)));
            }
        }

        Ok(Expr::object(pairs))
    }

    fn read_arrow_function(&self, node: Node) -> Result<Expr, ReadError> {
        let mut param_names = Vec::new();

        // Try "parameters" field first (for parenthesized params)
        if let Some(params) = node.child_by_field_name("parameters") {
            self.collect_params(params, &mut param_names);
        }
        // Try "parameter" field (for single unparenthesized param: x => ...)
        if let Some(param) = node.child_by_field_name("parameter") {
            if param.kind() == "identifier" {
                param_names.push(self.node_text(param).to_string());
            }
        }

        // Get body
        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("arrow_function missing body".into()))?;

        // Arrow function body can be expression or block
        let body = if body_node.kind() == "statement_block" {
            let block = self.read_block(body_node)?;
            match block {
                Stmt::Block(stmts) => stmts,
                other => vec![other],
            }
        } else {
            // Expression body - wrap in implicit return
            let expr = self.read_expr(body_node)?;
            vec![Stmt::return_stmt(Some(expr))]
        };

        Ok(Expr::Function(Box::new(Function::anonymous(
            param_names,
            body,
        ))))
    }

    fn read_function_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(n).to_string())
            .unwrap_or_default();

        let mut param_names = Vec::new();
        if let Some(params) = node.child_by_field_name("parameters") {
            self.collect_params(params, &mut param_names);
        }

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        if name.is_empty() {
            Ok(Expr::Function(Box::new(Function::anonymous(
                param_names,
                body,
            ))))
        } else {
            Ok(Expr::Function(Box::new(Function::new(
                name,
                param_names,
                body,
            ))))
        }
    }

    fn collect_params(&self, params: Node, param_names: &mut Vec<String>) {
        match params.kind() {
            "identifier" => {
                param_names.push(self.node_text(params).to_string());
            }
            "formal_parameters" => {
                let mut cursor = params.walk();
                for child in params.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        param_names.push(self.node_text(child).to_string());
                    } else if child.kind() == "required_parameter" {
                        if let Some(pattern) = child.child_by_field_name("pattern") {
                            param_names.push(self.node_text(pattern).to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn read_ternary(&self, node: Node) -> Result<Expr, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("ternary missing condition".into()))?;
        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("ternary missing consequence".into()))?;
        let alternative = node
            .child_by_field_name("alternative")
            .ok_or_else(|| ReadError::Parse("ternary missing alternative".into()))?;

        Ok(Expr::conditional(
            self.read_expr(condition)?,
            self.read_expr(consequence)?,
            self.read_expr(alternative)?,
        ))
    }

    fn read_expression_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                return Ok(Stmt::expr(self.read_expr(child)?));
            }
        }
        Ok(Stmt::expr(Expr::null()))
    }

    fn read_lexical_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        // Determine if it's let or const
        let is_const = self.node_text(node).starts_with("const");

        let mut declarations = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                declarations.push(self.read_variable_declarator(child, !is_const)?);
            }
        }

        if declarations.len() == 1 {
            Ok(declarations.remove(0))
        } else {
            Ok(Stmt::block(declarations))
        }
    }

    fn read_variable_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        // var x = 1; (treat as mutable let)
        let mut declarations = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                declarations.push(self.read_variable_declarator(child, true)?);
            }
        }

        if declarations.len() == 1 {
            Ok(declarations.remove(0))
        } else {
            Ok(Stmt::block(declarations))
        }
    }

    fn read_variable_declarator(&self, node: Node, mutable: bool) -> Result<Stmt, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("variable_declarator missing name".into()))?;
        let value = node.child_by_field_name("value");

        let name_str = self.node_text(name).to_string();
        let init = if let Some(val) = value {
            Some(self.read_expr(val)?)
        } else {
            None
        };

        if mutable {
            Ok(Stmt::let_decl(name_str, init))
        } else {
            Ok(Stmt::Let {
                name: name_str,
                init,
                mutable: false,
            })
        }
    }

    fn read_if_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("if_statement missing condition".into()))?;
        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("if_statement missing consequence".into()))?;
        let alternative = node.child_by_field_name("alternative");

        let cond_expr = self.read_expr(condition)?;
        let then_stmt = self.read_stmt(consequence)?.unwrap_or(Stmt::block(vec![]));

        let else_stmt = if let Some(alt) = alternative {
            self.read_else_clause(alt).ok()
        } else {
            None
        };

        Ok(Stmt::if_stmt(cond_expr, then_stmt, else_stmt))
    }

    fn read_else_clause(&self, node: Node) -> Result<Stmt, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                return self
                    .read_stmt(child)?
                    .ok_or_else(|| ReadError::Parse("empty else clause".into()));
            }
        }
        Ok(Stmt::block(vec![]))
    }

    fn read_while_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("while_statement missing condition".into()))?;
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("while_statement missing body".into()))?;

        let cond_expr = self.read_expr(condition)?;
        let body_stmt = self.read_stmt(body)?.unwrap_or(Stmt::block(vec![]));

        Ok(Stmt::while_loop(cond_expr, body_stmt))
    }

    fn read_for_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let initializer = node.child_by_field_name("initializer");
        let condition = node.child_by_field_name("condition");
        let increment = node.child_by_field_name("increment");
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("for_statement missing body".into()))?;

        let init = if let Some(init_node) = initializer {
            self.read_stmt(init_node)?
        } else {
            None
        };

        let test = if let Some(cond_node) = condition {
            Some(self.read_expr(cond_node)?)
        } else {
            None
        };

        let update = if let Some(incr_node) = increment {
            Some(self.read_expr(incr_node)?)
        } else {
            None
        };

        let body_stmt = self.read_stmt(body)?.unwrap_or(Stmt::block(vec![]));

        Ok(Stmt::for_loop(init, test, update, body_stmt))
    }

    fn read_for_in_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("for_in_statement missing left".into()))?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("for_in_statement missing right".into()))?;
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("for_in_statement missing body".into()))?;

        // Detect if this is "for...in" (object keys) or "for...of" (array/iterable values)
        let is_for_in = {
            let mut cursor = node.walk();
            let mut found_in = false;
            for child in node.children(&mut cursor) {
                let text = self.node_text(child);
                if text == "in" {
                    found_in = true;
                    break;
                } else if text == "of" {
                    break;
                }
            }
            found_in
        };

        let var_name = self.extract_for_variable(left)?;
        let right_expr = self.read_expr(right)?;
        let body_stmt = self.read_stmt(body)?.unwrap_or(Stmt::block(vec![]));

        // For "for...in", we iterate over obj.keys(obj)
        let iter_expr = if is_for_in {
            Expr::call(Expr::member(Expr::ident("obj"), "keys"), vec![right_expr])
        } else {
            right_expr
        };

        Ok(Stmt::for_in(var_name, iter_expr, body_stmt))
    }

    fn extract_for_variable(&self, node: Node) -> Result<String, ReadError> {
        match node.kind() {
            "identifier" => Ok(self.node_text(node).to_string()),
            "lexical_declaration" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "variable_declarator" {
                        if let Some(name) = child.child_by_field_name("name") {
                            return Ok(self.node_text(name).to_string());
                        }
                    }
                }
                Err(ReadError::Parse(
                    "for-of: could not extract variable name".into(),
                ))
            }
            _ => Err(ReadError::Unsupported(format!(
                "for-of variable type '{}'",
                node.kind()
            ))),
        }
    }

    fn read_switch_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| ReadError::Parse("switch_statement missing value".into()))?;
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("switch_statement missing body".into()))?;

        let value_expr = self.read_expr(value)?;

        // Collect cases and default
        let mut cases: Vec<(Expr, Vec<Stmt>)> = Vec::new();
        let mut default_body: Vec<Stmt> = Vec::new();

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "switch_case" => {
                    if let Some(case_value) = child.child_by_field_name("value") {
                        let case_expr = self.read_expr(case_value)?;
                        let mut body_stmts = Vec::new();

                        let mut inner_cursor = child.walk();
                        let mut past_colon = false;
                        for inner_child in child.children(&mut inner_cursor) {
                            if inner_child.kind() == ":" {
                                past_colon = true;
                                continue;
                            }
                            if past_colon && inner_child.is_named() {
                                if inner_child.kind() != "break_statement" {
                                    if let Some(stmt) = self.read_stmt(inner_child)? {
                                        body_stmts.push(stmt);
                                    }
                                }
                            }
                        }

                        cases.push((case_expr, body_stmts));
                    }
                }
                "switch_default" => {
                    let mut inner_cursor = child.walk();
                    let mut past_colon = false;
                    for inner_child in child.children(&mut inner_cursor) {
                        if inner_child.kind() == ":" {
                            past_colon = true;
                            continue;
                        }
                        if past_colon && inner_child.is_named() {
                            if inner_child.kind() != "break_statement" {
                                if let Some(stmt) = self.read_stmt(inner_child)? {
                                    default_body.push(stmt);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Build nested if-else from cases (reverse order to build from inside out)
        let default_stmt = if default_body.is_empty() {
            Stmt::block(vec![])
        } else if default_body.len() == 1 {
            default_body.into_iter().next().unwrap()
        } else {
            Stmt::block(default_body)
        };

        let result =
            cases
                .into_iter()
                .rev()
                .fold(default_stmt, |else_branch, (case_val, body_stmts)| {
                    let body_stmt = if body_stmts.is_empty() {
                        Stmt::block(vec![])
                    } else if body_stmts.len() == 1 {
                        body_stmts.into_iter().next().unwrap()
                    } else {
                        Stmt::block(body_stmts)
                    };

                    let condition = Expr::binary(value_expr.clone(), BinaryOp::Eq, case_val);

                    Stmt::if_stmt(condition, body_stmt, Some(else_branch))
                });

        Ok(result)
    }

    fn read_try_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("try_statement missing body".into()))?;

        let body_stmt = self.read_block(body)?;

        let handler = node.child_by_field_name("handler");
        let (catch_param, catch_body) = if let Some(h) = handler {
            let param = h
                .child_by_field_name("parameter")
                .map(|p| self.node_text(p).to_string());
            let catch_body_node = h
                .child_by_field_name("body")
                .ok_or_else(|| ReadError::Parse("catch_clause missing body".into()))?;
            (param, Some(self.read_block(catch_body_node)?))
        } else {
            (None, None)
        };

        let finalizer = node.child_by_field_name("finalizer");
        let finally_body = finalizer
            .and_then(|f| f.child_by_field_name("body"))
            .map(|f| self.read_block(f))
            .transpose()?;

        Ok(Stmt::try_catch(
            body_stmt,
            catch_param,
            catch_body,
            finally_body,
        ))
    }

    fn read_return_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() && child.kind() != "return" {
                let value = self.read_expr(child)?;
                return Ok(Stmt::return_stmt(Some(value)));
            }
        }
        Ok(Stmt::return_stmt(None))
    }

    fn read_block(&self, node: Node) -> Result<Stmt, ReadError> {
        let statements = self.read_block_stmts(node)?;
        Ok(Stmt::block(statements))
    }

    fn read_block_stmts(&self, node: Node) -> Result<Vec<Stmt>, ReadError> {
        let mut statements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                if let Some(stmt) = self.read_stmt(child)? {
                    statements.push(stmt);
                }
            }
        }

        Ok(statements)
    }

    fn read_function_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("function_declaration missing name".into()))?;

        let mut param_names = Vec::new();
        if let Some(params) = node.child_by_field_name("parameters") {
            self.collect_params(params, &mut param_names);
        }

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function_declaration missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        Ok(Stmt::function(Function::new(
            self.node_text(name),
            param_names,
            body,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_let() {
        let program = read_typescript("let x = 42;").unwrap();
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                name,
                init,
                mutable,
            } => {
                assert_eq!(name, "x");
                assert!(mutable);
                assert!(init.is_some());
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn test_binary_expr() {
        let program = read_typescript("1 + 2").unwrap();
        match &program.body[0] {
            Stmt::Expr(Expr::Binary { op, .. }) => {
                assert_eq!(op, &BinaryOp::Add);
            }
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_function_call() {
        let program = read_typescript("console.log('hello')").unwrap();
        match &program.body[0] {
            Stmt::Expr(Expr::Call { callee, args }) => {
                assert_eq!(args.len(), 1);
                match callee.as_ref() {
                    Expr::Member { .. } => {}
                    _ => panic!("expected Member expression"),
                }
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn test_arrow_function() {
        let program = read_typescript("const add = (a, b) => a + b;").unwrap();
        assert_eq!(program.body.len(), 1);
    }

    #[test]
    fn test_if_statement() {
        let program = read_typescript("if (x > 0) { return 1; } else { return 0; }").unwrap();
        match &program.body[0] {
            Stmt::If {
                test, alternate, ..
            } => {
                assert!(matches!(
                    test,
                    Expr::Binary {
                        op: BinaryOp::Gt,
                        ..
                    }
                ));
                assert!(alternate.is_some());
            }
            _ => panic!("expected If"),
        }
    }
}
