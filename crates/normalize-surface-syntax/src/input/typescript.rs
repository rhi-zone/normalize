//! Tree-sitter based TypeScript reader.

use crate::ir::{
    BinaryOp, ExportName, Expr, Function, ImportName, Method, Param, Pat, PatField, Program, Span,
    Stmt, TemplatePart, UnaryOp,
};
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
    let language = normalize_languages::parsers::grammar_loader()
        .get("typescript")
        .map_err(|e| ReadError::Parse(format!("load typescript grammar: {e}")))?;
    read_with_language(source, language)
}

/// Parse source into surface-syntax IR using the given tree-sitter language.
/// Used by language readers that share TypeScript's node-type grammar (e.g. JavaScript).
pub(crate) fn read_with_language(
    source: &str,
    language: tree_sitter::Language,
) -> Result<Program, ReadError> {
    let mut parser = Parser::new();
    parser
        .set_language(&language)
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
            if child.is_named()
                && let Some(stmt) = self.read_stmt(child)?
            {
                statements.push(stmt);
            }
        }

        Ok(Program::new(statements))
    }

    fn read_stmt(&self, node: Node) -> Result<Option<Stmt>, ReadError> {
        match node.kind() {
            // Empty statements (skip)
            "empty_statement" => Ok(None),

            // Comments — preserve as Stmt::Comment for documentation translation
            "comment" => {
                let raw = self.node_text(node);
                let span = Span::from_ts(node.start_position(), node.end_position());
                let stmt = if let Some(inner) = raw.strip_prefix("/**") {
                    // JSDoc block comment: strip /** and */
                    let content = inner.strip_suffix("*/").unwrap_or(inner).trim();
                    Stmt::comment_block(content)
                } else if let Some(inner) = raw.strip_prefix("/*") {
                    // Block comment: strip /* and */
                    let content = inner.strip_suffix("*/").unwrap_or(inner).trim();
                    Stmt::comment_block(content)
                } else if let Some(inner) = raw.strip_prefix("//") {
                    // Line comment: strip //
                    let content = inner.trim_start_matches('/').trim();
                    Stmt::comment_line(content)
                } else {
                    Stmt::comment_line(raw.trim())
                };
                Ok(Some(stmt.with_span(span)))
            }

            // TypeScript-only declarations (no runtime meaning, skip)
            "interface_declaration"
            | "type_alias_declaration"
            | "abstract_class_declaration"
            | "enum_declaration"
            | "ambient_declaration"
            | "module" => Ok(None),

            // Import/export statements — parse into first-class IR nodes
            "import_statement" => self.read_import_statement(node).map(Some),
            "export_statement" => self.read_export_statement(node).map(Some),

            // Statements
            "expression_statement" => self.read_expression_statement(node).map(Some),
            "lexical_declaration" => self.read_lexical_declaration(node).map(Some),
            "variable_declaration" => self.read_variable_declaration(node).map(Some),
            "if_statement" => self.read_if_statement(node).map(Some),
            "while_statement" => self.read_while_statement(node).map(Some),
            "do_statement" => self.read_do_while_statement(node).map(Some),
            "for_statement" => self.read_for_statement(node).map(Some),
            "for_in_statement" => self.read_for_in_statement(node).map(Some),
            "switch_statement" => self.read_switch_statement(node).map(Some),
            "try_statement" => self.read_try_statement(node).map(Some),
            "break_statement" => Ok(Some(Stmt::break_stmt())),
            "continue_statement" => Ok(Some(Stmt::continue_stmt())),
            "return_statement" => self.read_return_statement(node).map(Some),
            "statement_block" => self.read_block(node).map(Some),
            "function_declaration" => self.read_function_declaration(node).map(Some),
            "class_declaration" => self.read_class_declaration(node).map(Some),

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
            "new_expression" => self.read_new_expr(node),
            "member_expression" => self.read_member_expr(node),
            "subscript_expression" => self.read_subscript_expr(node),
            "array" => self.read_array(node),
            "object" => self.read_object(node),
            "template_string" => self.read_template_string(node),
            "arrow_function" => self.read_arrow_function(node),
            "function" => self.read_function_expr(node),
            "ternary_expression" => self.read_ternary(node),

            // await expr — lower to the inner expression (async/await is transparent at IR level)
            "await_expression" => self.read_await_expression(node),

            // class expression — lower to a function expression
            "class" => self.read_class_expr(node),

            // Type assertions - just pass through the inner expression
            "as_expression" => self.read_as_expression(node),
            "non_null_expression" => self.read_non_null_expression(node),
            // Type casts / satisfies
            "type_assertion" | "satisfies_expression" => {
                let inner = node
                    .named_child(0)
                    .ok_or_else(|| ReadError::Parse("type_assertion missing expression".into()))?;
                self.read_expr(inner)
            }

            // Spread element in array/call (e.g. [...arr] or f(...args)) — lower to the inner expr
            "spread_element" => {
                let inner = node
                    .named_child(0)
                    .ok_or_else(|| ReadError::Parse("spread_element missing expression".into()))?;
                self.read_expr(inner)
            }

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
        // NOTE: basic escape sequences only; full unicode escape handling not yet supported
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
        let mut parts: Vec<TemplatePart> = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                // String fragment between interpolations
                "string_fragment" | "template_fragment" => {
                    let text = self.node_text(child);
                    if !text.is_empty() {
                        parts.push(TemplatePart::Text(text.to_string()));
                    }
                }
                // Interpolation: ${...}
                "template_substitution" => {
                    // Find the expression inside the ${ }
                    if let Some(expr) = child.named_child(0) {
                        parts.push(TemplatePart::Expr(Box::new(self.read_expr(expr)?)));
                    }
                }
                // Skip the ` characters
                "`" => {}
                _ => {}
            }
        }

        Ok(Expr::TemplateLiteral(parts))
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
        let mut params = Vec::new();

        // Try "parameters" field first (for parenthesized params)
        if let Some(params_node) = node.child_by_field_name("parameters") {
            self.collect_params(params_node, &mut params);
        }
        // Try "parameter" field (for single unparenthesized param: x => ...)
        if let Some(param) = node.child_by_field_name("parameter")
            && param.kind() == "identifier"
        {
            params.push(Param::new(self.node_text(param)));
        }

        // Get return type annotation if present
        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| self.extract_type_annotation_text(n));

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

        let mut func = Function::anonymous(params, body);
        func.return_type = return_type;
        Ok(Expr::Function(Box::new(func)))
    }

    fn read_function_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(n).to_string())
            .unwrap_or_default();

        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            self.collect_params(params_node, &mut params);
        }

        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| self.extract_type_annotation_text(n));

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        let mut func = if name.is_empty() {
            Function::anonymous(params, body)
        } else {
            Function::new(name, params, body)
        };
        func.return_type = return_type;
        Ok(Expr::Function(Box::new(func)))
    }

    fn collect_params(&self, params: Node, out: &mut Vec<Param>) {
        match params.kind() {
            "identifier" => {
                out.push(Param::new(self.node_text(params)));
            }
            "formal_parameters" => {
                let mut cursor = params.walk();
                for child in params.children(&mut cursor) {
                    self.collect_param(child, out);
                }
            }
            _ => {}
        }
    }

    fn collect_param(&self, node: Node, out: &mut Vec<Param>) {
        match node.kind() {
            "identifier" => {
                out.push(Param::new(self.node_text(node)));
            }
            // TypeScript required_parameter: pattern with optional type annotation
            "required_parameter" | "optional_parameter" => {
                if let Some(pattern) = node.child_by_field_name("pattern") {
                    // Extract the type annotation from the parameter node (not the pattern)
                    let type_annotation = node
                        .child_by_field_name("type")
                        .map(|n| self.extract_type_annotation_text(n));

                    // If pattern is a simple identifier, create a typed param directly
                    if pattern.kind() == "identifier" {
                        let mut param = Param::new(self.node_text(pattern));
                        param.type_annotation = type_annotation;
                        out.push(param);
                    } else {
                        // Destructuring pattern: collect sub-params (no type annotation for each)
                        self.collect_param(pattern, out);
                    }
                }
            }
            // rest parameter: ...args
            "rest_pattern" => {
                // The child is the identifier (e.g. "args" in "...args")
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        out.push(Param::new(self.node_text(child)));
                        return;
                    }
                }
            }
            // object destructuring parameter: { a, b }
            "object_pattern" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "shorthand_property_identifier_pattern" => {
                            out.push(Param::new(self.node_text(child)));
                        }
                        "pair_pattern" => {
                            // { key: name } — use the value name
                            if let Some(val) = child.child_by_field_name("value")
                                && val.kind() == "identifier"
                            {
                                out.push(Param::new(self.node_text(val)));
                            }
                        }
                        "rest_pattern" => {
                            self.collect_param(child, out);
                        }
                        _ => {}
                    }
                }
            }
            // array destructuring parameter: [a, b]
            "array_pattern" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.is_named() && child.kind() != "," {
                        self.collect_param(child, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// Extract the type text from a `type_annotation` node (strips the leading `:`).
    fn extract_type_annotation_text(&self, node: Node) -> String {
        // type_annotation nodes have the form `: type_expr`
        // We want the text of the type expression child, not the whole node (which includes `:`)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                ":" => continue,
                _ if child.is_named() => {
                    return self.node_text(child).to_string();
                }
                _ => {}
            }
        }
        // Fallback: trim the leading `: ` from the raw text
        let raw = self.node_text(node);
        raw.trim_start_matches(':').trim().to_string()
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
        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("variable_declarator missing name".into()))?;
        let value = node.child_by_field_name("value");

        let init = if let Some(val) = value {
            Some(self.read_expr(val)?)
        } else {
            None
        };

        // Handle destructuring patterns: { a, b } = obj  or  [x, y] = arr
        match name_node.kind() {
            "object_pattern" | "array_pattern" => {
                let rhs = init.unwrap_or(Expr::null());
                let pat = self.read_pat(name_node)?;
                let span = Span::from_ts(node.start_position(), node.end_position());
                return Ok(Stmt::destructure(pat, rhs, mutable).with_span(span));
            }
            _ => {}
        }

        let name_str = self.node_text(name_node).to_string();
        // Extract type annotation from the declarator's `type` field
        // In TS: `const x: string = ...` — the variable_declarator has a `type_annotation` child
        let type_annotation = node
            .child_by_field_name("type")
            .map(|n| self.extract_type_annotation_text(n));
        Ok(Stmt::Let {
            name: name_str,
            init,
            mutable,
            type_annotation,
            span: None,
        })
    }

    /// Parse a tree-sitter pattern node into a `Pat` IR node.
    fn read_pat(&self, node: Node) -> Result<Pat, ReadError> {
        match node.kind() {
            "identifier" => Ok(Pat::ident(self.node_text(node))),

            "object_pattern" => {
                let mut fields = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "shorthand_property_identifier_pattern" => {
                            // { foo } — shorthand field
                            fields.push(PatField::shorthand(self.node_text(child)));
                        }
                        "pair_pattern" => {
                            // { key: pat } or { key: pat = default }
                            let key = child.child_by_field_name("key").ok_or_else(|| {
                                ReadError::Parse("pair_pattern missing key".into())
                            })?;
                            let val = child.child_by_field_name("value").ok_or_else(|| {
                                ReadError::Parse("pair_pattern missing value".into())
                            })?;
                            let key_str = self.node_text(key).to_string();
                            // val may be an identifier, nested pattern, or assignment_pattern
                            let (inner_val, default) = if val.kind() == "assignment_pattern" {
                                let lhs = val.child_by_field_name("left").ok_or_else(|| {
                                    ReadError::Parse("assignment_pattern missing left".into())
                                })?;
                                let rhs = val.child_by_field_name("right").ok_or_else(|| {
                                    ReadError::Parse("assignment_pattern missing right".into())
                                })?;
                                (lhs, Some(self.read_expr(rhs)?))
                            } else {
                                (val, None)
                            };
                            let pat = self.read_pat(inner_val)?;
                            let mut field = PatField::nested(key_str, pat);
                            if let Some(d) = default {
                                field = field.with_default(d);
                            }
                            fields.push(field);
                        }
                        "assignment_pattern" => {
                            // shorthand with default: { foo = "bar" }
                            let lhs = child.child_by_field_name("left").ok_or_else(|| {
                                ReadError::Parse("assignment_pattern missing left".into())
                            })?;
                            let rhs = child.child_by_field_name("right").ok_or_else(|| {
                                ReadError::Parse("assignment_pattern missing right".into())
                            })?;
                            let key = self.node_text(lhs).to_string();
                            let default = self.read_expr(rhs)?;
                            fields.push(PatField::shorthand(key.as_str()).with_default(default));
                        }
                        "rest_pattern" => {
                            // { ...rest }
                            let inner = self.read_rest_pat_inner(child)?;
                            fields.push(PatField::nested("...", Pat::Rest(Box::new(inner))));
                        }
                        _ => {}
                    }
                }
                Ok(Pat::Object(fields))
            }

            "array_pattern" => {
                let mut elements: Vec<Option<Pat>> = Vec::new();
                let mut rest: Option<String> = None;
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();
                let mut i = 0;
                while i < children.len() {
                    let child = children[i];
                    match child.kind() {
                        "[" | "]" => {}
                        "," => {
                            // A bare comma with no preceding named child since last element =
                            // a hole. We detect holes by checking if the previous child was
                            // also a comma (or the opening bracket).
                            if i == 0
                                || children[i - 1].kind() == "["
                                || children[i - 1].kind() == ","
                            {
                                elements.push(None);
                            }
                        }
                        "rest_pattern" => {
                            // [...rest] — extract the identifier name
                            let mut inner_cur = child.walk();
                            for inner in child.children(&mut inner_cur) {
                                if inner.kind() == "identifier" {
                                    rest = Some(self.node_text(inner).to_string());
                                    break;
                                }
                            }
                        }
                        _ if child.is_named() => {
                            elements.push(Some(self.read_pat(child)?));
                        }
                        _ => {}
                    }
                    i += 1;
                }
                Ok(Pat::Array(elements, rest))
            }

            "rest_pattern" => {
                let inner = self.read_rest_pat_inner(node)?;
                Ok(Pat::Rest(Box::new(inner)))
            }

            other => Err(ReadError::Unsupported(format!("pattern type '{}'", other))),
        }
    }

    /// Extract the inner `Pat` from a `rest_pattern` node (the `...x` part → `Pat::Ident("x")`).
    fn read_rest_pat_inner(&self, node: Node) -> Result<Pat, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                return self.read_pat(child);
            }
        }
        Err(ReadError::Parse("rest_pattern is empty".into()))
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
                    if child.kind() == "variable_declarator"
                        && let Some(name) = child.child_by_field_name("name")
                    {
                        return Ok(self.node_text(name).to_string());
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
                            if past_colon
                                && inner_child.is_named()
                                && inner_child.kind() != "break_statement"
                                && let Some(stmt) = self.read_stmt(inner_child)?
                            {
                                body_stmts.push(stmt);
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
                        if past_colon
                            && inner_child.is_named()
                            && inner_child.kind() != "break_statement"
                            && let Some(stmt) = self.read_stmt(inner_child)?
                        {
                            default_body.push(stmt);
                        }
                    }
                }
                _ => {}
            }
        }

        // Build nested if-else from cases (reverse order to build from inside out)
        let default_stmt = if default_body.len() == 1 {
            default_body.remove(0)
        } else if default_body.is_empty() {
            Stmt::block(vec![])
        } else {
            Stmt::block(default_body)
        };

        let result = cases.into_iter().rev().fold(
            default_stmt,
            |else_branch, (case_val, mut body_stmts)| {
                let body_stmt = if body_stmts.len() == 1 {
                    body_stmts.remove(0)
                } else if body_stmts.is_empty() {
                    Stmt::block(vec![])
                } else {
                    Stmt::block(body_stmts)
                };

                let condition = Expr::binary(value_expr.clone(), BinaryOp::Eq, case_val);

                Stmt::if_stmt(condition, body_stmt, Some(else_branch))
            },
        );

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
            if child.is_named()
                && let Some(stmt) = self.read_stmt(child)?
            {
                statements.push(stmt);
            }
        }

        Ok(statements)
    }

    fn read_function_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("function_declaration missing name".into()))?;

        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            self.collect_params(params_node, &mut params);
        }

        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| self.extract_type_annotation_text(n));

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function_declaration missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        let mut func = Function::new(self.node_text(name), params, body);
        func.return_type = return_type;
        Ok(Stmt::function(func))
    }

    /// Parse a class declaration into a first-class `Stmt::Class` IR node.
    ///
    /// `class Foo extends Bar { constructor(x) { ... } method() { ... } }` becomes:
    /// `Stmt::Class { name: "Foo", extends: Some("Bar"), methods: [...] }`
    fn read_class_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        let name_node = node.child_by_field_name("name");
        let class_name = name_node
            .map(|n| self.node_text(n).to_string())
            .unwrap_or_else(|| "__class__".to_string());

        // Walk class_declaration children to find class_heritage → extends_clause → type_identifier.
        // `child_by_field_name("heritage")` may not match the grammar's field name.
        let extends = {
            let mut cur = node.walk();
            let heritage = node
                .children(&mut cur)
                .find(|c| c.kind() == "class_heritage");
            heritage.and_then(|h| {
                let mut c2 = h.walk();
                let ext_clause = h.children(&mut c2).find(|c| c.kind() == "extends_clause");
                ext_clause.and_then(|ec| {
                    let mut c3 = ec.walk();
                    ec.children(&mut c3)
                        .find(|c| matches!(c.kind(), "type_identifier" | "identifier"))
                        .map(|c| self.node_text(c).to_string())
                })
            })
        };

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("class_declaration missing body".into()))?;

        let methods = self.read_class_body(body)?;
        let span = Span::from_ts(node.start_position(), node.end_position());
        Ok(Stmt::class(class_name, extends, methods).with_span(span))
    }

    /// Parse a class expression — lower to a function expression (constructor only).
    ///
    /// Class expressions remain lowered to function expressions because `Expr` has no
    /// `Class` variant — they appear inline and the constructor is the best approximation.
    fn read_class_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(n).to_string())
            .unwrap_or_default();

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("class expression missing body".into()))?;

        // Find the constructor method to use as the function body
        let (params, ctor_body) = self.extract_constructor(body)?;

        Ok(Expr::Function(Box::new(Function::new(
            name, params, ctor_body,
        ))))
    }

    /// Parse a class body node into a list of `Method` IR nodes.
    fn read_class_body(&self, body: Node) -> Result<Vec<Method>, ReadError> {
        let mut methods = Vec::new();
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            if child.kind() == "method_definition" {
                let name_node = match child.child_by_field_name("name") {
                    Some(n) => n,
                    None => continue,
                };
                let method_name = self.node_text(name_node).to_string();

                // Detect `static` keyword
                let is_static = {
                    let mut c2 = child.walk();
                    child
                        .children(&mut c2)
                        .any(|ch| ch.kind() == "static" || ch.kind() == "static_keyword")
                };

                let mut params = Vec::new();
                if let Some(p) = child.child_by_field_name("parameters") {
                    self.collect_params(p, &mut params);
                }

                let body_stmts = child
                    .child_by_field_name("body")
                    .map(|b| self.read_block_stmts(b))
                    .transpose()?
                    .unwrap_or_default();

                let return_type = child
                    .child_by_field_name("return_type")
                    .map(|n| self.extract_type_annotation_text(n));

                let mut method = Method::new(method_name, params, body_stmts);
                method.is_static = is_static;
                method.return_type = return_type;
                methods.push(method);
            }
        }

        Ok(methods)
    }

    /// Extract constructor params and body from a class body node.
    fn extract_constructor(&self, body: Node) -> Result<(Vec<Param>, Vec<Stmt>), ReadError> {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "method_definition" {
                let name_node = child.child_by_field_name("name");
                if name_node.map(|n| self.node_text(n)) == Some("constructor") {
                    let mut params = Vec::new();
                    if let Some(p) = child.child_by_field_name("parameters") {
                        self.collect_params(p, &mut params);
                    }
                    let body_stmts = child
                        .child_by_field_name("body")
                        .map(|b| self.read_block_stmts(b))
                        .transpose()?
                        .unwrap_or_default();
                    return Ok((params, body_stmts));
                }
            }
        }
        // No constructor: empty body
        Ok((vec![], vec![]))
    }

    /// Parse `import_statement` into `Stmt::Import`.
    ///
    /// Handles:
    /// - `import { foo, bar as b } from './module'`  → named imports
    /// - `import * as ns from 'other'`               → namespace import
    /// - `import DefaultExport from 'default-mod'`   → default import
    /// - `import './side-effect'`                    → side-effect import (empty names)
    fn read_import_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        // Source string: the `from 'source'` part (last string child)
        let source = self.extract_import_source(node).unwrap_or_default();

        let mut names: Vec<ImportName> = Vec::new();

        // Walk children to find `import_clause` (field name may vary by grammar version)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_clause" {
                self.collect_import_clause(child, &mut names);
                break;
            }
        }

        let span = Span::from_ts(node.start_position(), node.end_position());
        Ok(Stmt::import(source, names).with_span(span))
    }

    /// Extract the `'source'` string from an import/export statement node.
    fn extract_import_source(&self, node: Node) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                let raw = self.node_text(child);
                // Strip surrounding quotes
                let inner = raw.trim_matches('"').trim_matches('\'');
                return Some(inner.to_string());
            }
        }
        None
    }

    /// Collect import specifiers from an `import_clause` node into `names`.
    fn collect_import_clause(&self, clause: Node, names: &mut Vec<ImportName>) {
        let mut cursor = clause.walk();
        for child in clause.children(&mut cursor) {
            match child.kind() {
                // Default import: `import Foo from '...'` — the clause itself is an identifier
                "identifier" => {
                    names.push(ImportName::default(self.node_text(child)));
                }
                // Namespace import: `import * as ns from '...'`
                "namespace_import" => {
                    let mut c2 = child.walk();
                    if let Some(alias) = child.children(&mut c2).find(|c| c.kind() == "identifier")
                    {
                        names.push(ImportName::namespace(self.node_text(alias)));
                    }
                }
                // Named imports: `import { foo, bar as b } from '...'`
                "named_imports" => {
                    let mut c2 = child.walk();
                    for specifier in child.children(&mut c2) {
                        if specifier.kind() == "import_specifier" {
                            self.collect_import_specifier(specifier, names);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Parse a single `import_specifier` node and push to `names`.
    fn collect_import_specifier(&self, node: Node, names: &mut Vec<ImportName>) {
        // `import_specifier` children: identifier [as identifier]
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|c| c.kind() == "identifier")
            .collect();

        match children.len() {
            1 => {
                names.push(ImportName::named(self.node_text(children[0])));
            }
            2 => {
                // `foo as bar`
                names.push(ImportName::aliased(
                    self.node_text(children[0]),
                    self.node_text(children[1]),
                ));
            }
            _ => {}
        }
    }

    /// Parse `export_statement` into `Stmt::Export`.
    ///
    /// Handles:
    /// - `export { foo, bar as baz }`         → named export
    /// - `export { x } from './re-export'`    → re-export
    /// - `export default expr`                → skip (no IR representation yet)
    /// - `export class Foo { ... }`           → emit the contained declaration
    fn read_export_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let span = Span::from_ts(node.start_position(), node.end_position());

        // Check for `export_clause` (named exports)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "export_clause" => {
                    let mut names: Vec<ExportName> = Vec::new();
                    let mut c2 = child.walk();
                    for specifier in child.children(&mut c2) {
                        if specifier.kind() == "export_specifier" {
                            self.collect_export_specifier(specifier, &mut names);
                        }
                    }
                    let source = self.extract_import_source(node);
                    return Ok(Stmt::export(names, source).with_span(span));
                }
                // `export class Foo { ... }` — parse the class declaration
                "class_declaration" => {
                    return self.read_class_declaration(child);
                }
                // `export function foo() { ... }` — parse the function
                "function_declaration" => {
                    return self.read_function_declaration(child);
                }
                // `export default expr` — skip (no IR repr for default export value)
                "default" => {
                    return Ok(Stmt::export(vec![], None).with_span(span));
                }
                _ => {}
            }
        }

        // Fallback: empty export
        Ok(Stmt::export(vec![], None).with_span(span))
    }

    /// Parse a single `export_specifier` node and push to `names`.
    fn collect_export_specifier(&self, node: Node, names: &mut Vec<ExportName>) {
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|c| c.kind() == "identifier")
            .collect();

        match children.len() {
            1 => {
                names.push(ExportName::named(self.node_text(children[0])));
            }
            2 => {
                names.push(ExportName::aliased(
                    self.node_text(children[0]),
                    self.node_text(children[1]),
                ));
            }
            _ => {}
        }
    }

    /// Lower `await expr` → the inner expression (async/await is transparent at IR level).
    fn read_await_expression(&self, node: Node) -> Result<Expr, ReadError> {
        let inner = node
            .named_child(0)
            .ok_or_else(|| ReadError::Parse("await_expression missing expression".into()))?;
        self.read_expr(inner)
    }

    /// Lower `new Foo(args)` → `Foo(args)` (constructor call).
    fn read_new_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let constructor = node
            .child_by_field_name("constructor")
            .ok_or_else(|| ReadError::Parse("new_expression missing constructor".into()))?;
        let callee = self.read_expr(constructor)?;

        let mut args = Vec::new();
        if let Some(arguments) = node.child_by_field_name("arguments") {
            let mut cursor = arguments.walk();
            for child in arguments.children(&mut cursor) {
                if child.is_named() {
                    args.push(self.read_expr(child)?);
                }
            }
        }

        Ok(Expr::call(callee, args))
    }

    fn read_do_while_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        // do { body } while (cond)  →  { body; while (cond) { body } }
        // Simplified: lower as while loop (execute body at least once is semantics,
        // but at the IR level we just model it as a while loop for simplicity).
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("do_statement missing condition".into()))?;
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("do_statement missing body".into()))?;

        let cond_expr = self.read_expr(condition)?;
        let body_stmt = self.read_stmt(body)?.unwrap_or(Stmt::block(vec![]));

        Ok(Stmt::while_loop(cond_expr, body_stmt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_let() -> Result<(), ReadError> {
        let program = read_typescript("let x = 42;")?;
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
    fn test_binary_expr() -> Result<(), ReadError> {
        let program = read_typescript("1 + 2")?;
        match &program.body[0] {
            Stmt::Expr(Expr::Binary { op, .. }) => {
                assert_eq!(op, &BinaryOp::Add);
            }
            _ => panic!("expected Binary"),
        }
        Ok(())
    }

    #[test]
    fn test_function_call() -> Result<(), ReadError> {
        let program = read_typescript("console.log('hello')")?;
        match &program.body[0] {
            Stmt::Expr(Expr::Call { callee, args, .. }) => {
                assert_eq!(args.len(), 1);
                match callee.as_ref() {
                    Expr::Member { .. } => {}
                    _ => panic!("expected Member expression"),
                }
            }
            _ => panic!("expected Call"),
        }
        Ok(())
    }

    #[test]
    fn test_arrow_function() -> Result<(), ReadError> {
        let program = read_typescript("const add = (a, b) => a + b;")?;
        assert_eq!(program.body.len(), 1);
        Ok(())
    }

    #[test]
    fn test_if_statement() -> Result<(), ReadError> {
        let program = read_typescript("if (x > 0) { return 1; } else { return 0; }")?;
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
        Ok(())
    }

    #[test]
    fn test_class_declaration() -> Result<(), ReadError> {
        let program = read_typescript(
            "class Animal { constructor(name) { this.name = name; } speak() { return 1; } }",
        )?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Class {
                name,
                extends,
                methods,
                ..
            } => {
                assert_eq!(name, "Animal");
                assert!(extends.is_none());
                assert_eq!(methods.len(), 2);
                assert_eq!(methods[0].name, "constructor");
                assert_eq!(methods[1].name, "speak");
            }
            _ => panic!("expected Class, got {:?}", program.body[0]),
        }
        Ok(())
    }

    #[test]
    fn test_class_extends() -> Result<(), ReadError> {
        let program = read_typescript("class Dog extends Animal { speak() { return 2; } }")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Class {
                name,
                extends,
                methods,
                ..
            } => {
                assert_eq!(name, "Dog");
                assert_eq!(extends.as_deref(), Some("Animal"));
                assert_eq!(methods.len(), 1);
            }
            _ => panic!("expected Class"),
        }
        Ok(())
    }

    #[test]
    fn test_import_named() -> Result<(), ReadError> {
        let program = read_typescript("import { foo, bar as b } from './module';")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Import { source, names, .. } => {
                assert_eq!(source, "./module");
                assert_eq!(names.len(), 2);
                assert_eq!(names[0].name, "foo");
                assert!(names[0].alias.is_none());
                assert_eq!(names[1].name, "bar");
                assert_eq!(names[1].alias.as_deref(), Some("b"));
            }
            _ => panic!("expected Import"),
        }
        Ok(())
    }

    #[test]
    fn test_import_namespace() -> Result<(), ReadError> {
        let program = read_typescript("import * as ns from 'other';")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Import { source, names, .. } => {
                assert_eq!(source, "other");
                assert_eq!(names.len(), 1);
                assert!(names[0].is_namespace);
                assert_eq!(names[0].alias.as_deref(), Some("ns"));
            }
            _ => panic!("expected Import"),
        }
        Ok(())
    }

    #[test]
    fn test_export_named() -> Result<(), ReadError> {
        let program = read_typescript("export { foo, bar as baz };")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Export { names, source, .. } => {
                assert_eq!(names.len(), 2);
                assert_eq!(names[0].name, "foo");
                assert_eq!(names[1].name, "bar");
                assert_eq!(names[1].alias.as_deref(), Some("baz"));
                assert!(source.is_none());
            }
            _ => panic!("expected Export"),
        }
        Ok(())
    }

    #[test]
    fn test_export_reexport() -> Result<(), ReadError> {
        let program = read_typescript("export { x } from './other';")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Export { names, source, .. } => {
                assert_eq!(names.len(), 1);
                assert_eq!(names[0].name, "x");
                assert_eq!(source.as_deref(), Some("./other"));
            }
            _ => panic!("expected Export"),
        }
        Ok(())
    }

    #[test]
    fn test_interface_declaration_skipped() -> Result<(), ReadError> {
        let program = read_typescript("interface Foo { bar: string; }")?;
        // Interface has no runtime meaning — should produce no statements
        assert_eq!(program.body.len(), 0);
        Ok(())
    }

    #[test]
    fn test_type_annotation_on_variable() -> Result<(), ReadError> {
        let program = read_typescript("const x: string = 'hello';")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                name,
                type_annotation,
                ..
            } => {
                assert_eq!(name, "x");
                assert_eq!(type_annotation.as_deref(), Some("string"));
            }
            _ => panic!("expected Let"),
        }
        Ok(())
    }

    #[test]
    fn test_object_destructuring_ir() -> Result<(), ReadError> {
        let program = read_typescript("const { a, b } = obj;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Destructure { pat, mutable, .. } => {
                assert!(!mutable);
                match pat {
                    Pat::Object(fields) => {
                        assert_eq!(fields.len(), 2);
                        assert_eq!(fields[0].key, "a");
                        assert!(matches!(&fields[0].pat, Pat::Ident(n) if n == "a"));
                        assert_eq!(fields[1].key, "b");
                        assert!(matches!(&fields[1].pat, Pat::Ident(n) if n == "b"));
                    }
                    _ => panic!("expected Pat::Object, got {:?}", pat),
                }
            }
            _ => panic!("expected Destructure, got {:?}", program.body[0]),
        }
        Ok(())
    }

    #[test]
    fn test_object_destructuring_renamed() -> Result<(), ReadError> {
        let program = read_typescript("const { b: c } = obj;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Destructure { pat, .. } => match pat {
                Pat::Object(fields) => {
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0].key, "b");
                    assert!(matches!(&fields[0].pat, Pat::Ident(n) if n == "c"));
                }
                _ => panic!("expected Pat::Object"),
            },
            _ => panic!("expected Destructure"),
        }
        Ok(())
    }

    #[test]
    fn test_array_destructuring_ir() -> Result<(), ReadError> {
        let program = read_typescript("const [x, y] = arr;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Destructure { pat, mutable, .. } => {
                assert!(!mutable);
                match pat {
                    Pat::Array(elements, rest) => {
                        assert_eq!(elements.len(), 2);
                        assert!(matches!(&elements[0], Some(Pat::Ident(n)) if n == "x"));
                        assert!(matches!(&elements[1], Some(Pat::Ident(n)) if n == "y"));
                        assert!(rest.is_none());
                    }
                    _ => panic!("expected Pat::Array, got {:?}", pat),
                }
            }
            _ => panic!("expected Destructure, got {:?}", program.body[0]),
        }
        Ok(())
    }

    #[test]
    fn test_array_destructuring_with_rest() -> Result<(), ReadError> {
        let program = read_typescript("const [first, ...rest] = arr;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Destructure { pat, .. } => match pat {
                Pat::Array(elements, rest) => {
                    assert_eq!(elements.len(), 1);
                    assert!(matches!(&elements[0], Some(Pat::Ident(n)) if n == "first"));
                    assert_eq!(rest.as_deref(), Some("rest"));
                }
                _ => panic!("expected Pat::Array"),
            },
            _ => panic!("expected Destructure"),
        }
        Ok(())
    }

    #[test]
    fn test_object_destructuring_round_trip() -> Result<(), ReadError> {
        use crate::output::typescript::TypeScriptWriter;
        let src = "const { a, b: c } = obj;";
        let program = read_typescript(src)?;
        let out = TypeScriptWriter::emit(&program);
        assert_eq!(out.trim(), "const { a, b: c } = obj;");
        Ok(())
    }

    #[test]
    fn test_array_destructuring_round_trip() -> Result<(), ReadError> {
        use crate::output::typescript::TypeScriptWriter;
        let src = "const [x, y] = arr;";
        let program = read_typescript(src)?;
        let out = TypeScriptWriter::emit(&program);
        assert_eq!(out.trim(), "const [x, y] = arr;");
        Ok(())
    }

    #[test]
    fn test_array_rest_destructuring_round_trip() -> Result<(), ReadError> {
        use crate::output::typescript::TypeScriptWriter;
        let src = "const [first, ...rest] = arr;";
        let program = read_typescript(src)?;
        let out = TypeScriptWriter::emit(&program);
        assert_eq!(out.trim(), "const [first, ...rest] = arr;");
        Ok(())
    }

    #[test]
    fn test_object_rest_destructuring_round_trip() -> Result<(), ReadError> {
        use crate::output::typescript::TypeScriptWriter;
        let src = "const { a, ...rest } = obj;";
        let program = read_typescript(src)?;
        let out = TypeScriptWriter::emit(&program);
        assert_eq!(out.trim(), "const { a, ...rest } = obj;");
        Ok(())
    }

    #[test]
    fn test_rest_params() -> Result<(), ReadError> {
        let program = read_typescript("function sum(...args) { return 1; }")?;
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

    #[test]
    fn test_typed_params_and_return_type() -> Result<(), ReadError> {
        let program =
            read_typescript("function greet(name: string, age: number): string { return name; }")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Function(f) => {
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.params[0].name, "name");
                assert_eq!(f.params[0].type_annotation.as_deref(), Some("string"));
                assert_eq!(f.params[1].name, "age");
                assert_eq!(f.params[1].type_annotation.as_deref(), Some("number"));
                assert_eq!(f.return_type.as_deref(), Some("string"));
            }
            _ => panic!("expected Function"),
        }
        Ok(())
    }

    #[test]
    fn test_typed_variable_declaration() -> Result<(), ReadError> {
        let program = read_typescript("const x: number = 42;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                name,
                type_annotation,
                ..
            } => {
                assert_eq!(name, "x");
                assert_eq!(type_annotation.as_deref(), Some("number"));
            }
            _ => panic!("expected Let"),
        }
        Ok(())
    }

    #[test]
    fn test_template_literal() -> Result<(), ReadError> {
        let program = read_typescript("const msg = `Hello ${name}!`;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                init: Some(Expr::TemplateLiteral(parts)),
                ..
            } => {
                assert_eq!(parts.len(), 3);
                assert!(matches!(&parts[0], TemplatePart::Text(t) if t == "Hello "));
                assert!(matches!(&parts[1], TemplatePart::Expr(_)));
                assert!(matches!(&parts[2], TemplatePart::Text(t) if t == "!"));
            }
            _ => panic!("expected Let with TemplateLiteral"),
        }
        Ok(())
    }

    #[test]
    fn test_template_literal_no_interp() -> Result<(), ReadError> {
        let program = read_typescript("const s = `plain text`;")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                init: Some(Expr::TemplateLiteral(parts)),
                ..
            } => {
                assert_eq!(parts.len(), 1);
                assert!(matches!(&parts[0], TemplatePart::Text(t) if t == "plain text"));
            }
            _ => panic!("expected Let with TemplateLiteral"),
        }
        Ok(())
    }

    #[test]
    fn test_await_expression() -> Result<(), ReadError> {
        let program = read_typescript("async function f() { const x = await fetch(url); }")?;
        assert_eq!(program.body.len(), 1);
        // The await should be transparent — x gets assigned the result of fetch(url)
        match &program.body[0] {
            Stmt::Function(f) => {
                assert_eq!(f.name, "f");
                assert!(!f.body.is_empty());
            }
            _ => panic!("expected Function"),
        }
        Ok(())
    }

    #[test]
    fn test_async_arrow_function() -> Result<(), ReadError> {
        let program = read_typescript("const f = async (x) => await doSomething(x);")?;
        assert_eq!(program.body.len(), 1);
        Ok(())
    }

    #[test]
    fn test_new_expression() -> Result<(), ReadError> {
        let program = read_typescript("const x = new Foo(1, 2);")?;
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let {
                init: Some(Expr::Call { .. }),
                ..
            } => {}
            _ => panic!("expected Let with Call init"),
        }
        Ok(())
    }

    #[test]
    fn test_line_comment_preserved() -> Result<(), ReadError> {
        let program = read_typescript("// This is a comment\nconst x = 1;")?;
        assert_eq!(program.body.len(), 2);
        match &program.body[0] {
            Stmt::Comment { text, block, .. } => {
                assert_eq!(text, "This is a comment");
                assert!(!block);
            }
            _ => panic!("expected Comment"),
        }
        Ok(())
    }

    #[test]
    fn test_block_comment_preserved() -> Result<(), ReadError> {
        let program = read_typescript("/* block comment */\nconst x = 1;")?;
        assert_eq!(program.body.len(), 2);
        match &program.body[0] {
            Stmt::Comment { text, block, .. } => {
                assert_eq!(text, "block comment");
                assert!(*block);
            }
            _ => panic!("expected Comment"),
        }
        Ok(())
    }

    #[test]
    fn test_jsdoc_comment_preserved() -> Result<(), ReadError> {
        let src = "/** Adds two numbers */\nfunction add(a, b) { return a + b; }";
        let program = read_typescript(src)?;
        assert_eq!(program.body.len(), 2);
        match &program.body[0] {
            Stmt::Comment { text, block, .. } => {
                assert_eq!(text, "Adds two numbers");
                assert!(*block);
            }
            _ => panic!("expected Comment"),
        }
        Ok(())
    }
}
