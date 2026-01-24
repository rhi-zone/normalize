//! Tree-sitter based Python reader.

use crate::ir::*;
use crate::traits::{ReadError, Reader};
use tree_sitter::{Node, Parser, Tree};

/// Static instance of the Python reader for registry.
pub static PYTHON_READER: PythonReader = PythonReader;

/// Python reader using tree-sitter.
pub struct PythonReader;

impl Reader for PythonReader {
    fn language(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn read(&self, source: &str) -> Result<Program, ReadError> {
        read_python(source)
    }
}

/// Parse Python source into surface-syntax IR.
pub fn read_python(source: &str) -> Result<Program, ReadError> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_python::language().into())
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
            // Comments
            "comment" => Ok(None),

            // Expression statements
            "expression_statement" => {
                let expr_node = node
                    .child(0)
                    .ok_or_else(|| ReadError::Parse("expression_statement has no child".into()))?;
                Ok(Some(Stmt::expr(self.read_expr(expr_node)?)))
            }

            // Assignment (x = value)
            "assignment" => self.read_assignment(node).map(Some),

            // Augmented assignment (x += value)
            "augmented_assignment" => self.read_augmented_assignment(node).map(Some),

            // Control flow
            "if_statement" => self.read_if_statement(node).map(Some),
            "while_statement" => self.read_while_statement(node).map(Some),
            "for_statement" => self.read_for_statement(node).map(Some),

            // Return
            "return_statement" => self.read_return_statement(node).map(Some),

            // Break/Continue
            "break_statement" => Ok(Some(Stmt::break_stmt())),
            "continue_statement" => Ok(Some(Stmt::continue_stmt())),

            // Pass (no-op, skip)
            "pass_statement" => Ok(None),

            // Function definition
            "function_definition" => self.read_function_definition(node).map(Some),

            // Imports (skip for now)
            "import_statement" | "import_from_statement" => Ok(None),

            // Class (skip for now)
            "class_definition" => Ok(None),

            // Try/except (skip for now)
            "try_statement" => Ok(None),

            // With (skip for now)
            "with_statement" => Ok(None),

            // Decorated definition
            "decorated_definition" => {
                // Get the inner definition
                if let Some(def) = node.child_by_field_name("definition") {
                    self.read_stmt(def)
                } else {
                    Ok(None)
                }
            }

            // Expression nodes at statement level (bare calls, etc.)
            "call"
            | "binary_operator"
            | "comparison_operator"
            | "boolean_operator"
            | "identifier"
            | "attribute" => Ok(Some(Stmt::expr(self.read_expr(node)?))),

            _ => {
                // Debug: print unknown node kind
                // eprintln!("Unknown Python statement kind: {}", node.kind());
                Ok(None)
            }
        }
    }

    fn read_assignment(&self, node: Node) -> Result<Stmt, ReadError> {
        // Python assignment: left = right
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("assignment missing left".into()))?;

        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("assignment missing right".into()))?;

        let name = self.node_text(left);
        let value = self.read_expr(right)?;

        // Treat as let declaration (Python doesn't distinguish)
        Ok(Stmt::let_decl(name, Some(value)))
    }

    fn read_augmented_assignment(&self, node: Node) -> Result<Stmt, ReadError> {
        // x += 1 becomes x = x + 1
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("augmented_assignment missing left".into()))?;

        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("augmented_assignment missing right".into()))?;

        let op_node = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("augmented_assignment missing operator".into()))?;

        let name = self.node_text(left);
        let op_text = self.node_text(op_node);
        let op = match op_text {
            "+=" => BinaryOp::Add,
            "-=" => BinaryOp::Sub,
            "*=" => BinaryOp::Mul,
            "/=" => BinaryOp::Div,
            "%=" => BinaryOp::Mod,
            _ => {
                return Err(ReadError::Parse(format!(
                    "unknown augmented op: {}",
                    op_text
                )));
            }
        };

        let rhs = self.read_expr(right)?;
        let value = Expr::binary(Expr::ident(name), op, rhs);

        Ok(Stmt::expr(Expr::assign(Expr::ident(name), value)))
    }

    fn read_if_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("if missing condition".into()))?;

        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("if missing consequence".into()))?;

        let test = self.read_expr(condition)?;
        let consequent = self.read_block(consequence)?;

        // Check for else/elif
        let alternate = if let Some(alt) = node.child_by_field_name("alternative") {
            match alt.kind() {
                "else_clause" => {
                    if let Some(body) = alt.child_by_field_name("body") {
                        Some(self.read_block(body)?)
                    } else {
                        None
                    }
                }
                "elif_clause" => {
                    // Treat elif as nested if
                    Some(self.read_elif_clause(alt)?)
                }
                _ => None,
            }
        } else {
            None
        };

        Ok(Stmt::if_stmt(test, consequent, alternate))
    }

    fn read_elif_clause(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("elif missing condition".into()))?;

        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("elif missing consequence".into()))?;

        let test = self.read_expr(condition)?;
        let consequent = self.read_block(consequence)?;

        let alternate = if let Some(alt) = node.child_by_field_name("alternative") {
            match alt.kind() {
                "else_clause" => {
                    if let Some(body) = alt.child_by_field_name("body") {
                        Some(self.read_block(body)?)
                    } else {
                        None
                    }
                }
                "elif_clause" => Some(self.read_elif_clause(alt)?),
                _ => None,
            }
        } else {
            None
        };

        Ok(Stmt::if_stmt(test, consequent, alternate))
    }

    fn read_while_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("while missing condition".into()))?;

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("while missing body".into()))?;

        let test = self.read_expr(condition)?;
        let body_stmt = self.read_block(body)?;

        Ok(Stmt::while_loop(test, body_stmt))
    }

    fn read_for_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("for missing left".into()))?;

        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("for missing right".into()))?;

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("for missing body".into()))?;

        let variable = self.node_text(left).to_string();
        let iterable = self.read_expr(right)?;
        let body_stmt = self.read_block(body)?;

        Ok(Stmt::for_in(variable, iterable, body_stmt))
    }

    fn read_return_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        // Python grammar: return_statement has child expression without field name
        // Find first named child that isn't the "return" keyword
        let mut cursor = node.walk();
        let expr_node = node
            .children(&mut cursor)
            .find(|c| c.is_named() && c.kind() != "return");
        let expr = expr_node.map(|n| self.read_expr(n)).transpose()?;
        Ok(Stmt::return_stmt(expr))
    }

    fn read_function_definition(&self, node: Node) -> Result<Stmt, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("function missing name".into()))?;

        let params = node.child_by_field_name("parameters");
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function missing body".into()))?;

        let fn_name = self.node_text(name).to_string();
        let fn_params = params
            .map(|p| self.read_parameters(p))
            .transpose()?
            .unwrap_or_default();
        let fn_body = self.read_block_stmts(body)?;

        Ok(Stmt::function(Function::new(fn_name, fn_params, fn_body)))
    }

    fn read_parameters(&self, node: Node) -> Result<Vec<String>, ReadError> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(self.node_text(child).to_string());
                }
                "default_parameter" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        params.push(self.node_text(name).to_string());
                    }
                }
                "typed_parameter" | "typed_default_parameter" => {
                    // Get just the name, ignore type annotation
                    if let Some(name) = child.child(0) {
                        if name.kind() == "identifier" {
                            params.push(self.node_text(name).to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(params)
    }

    fn read_block(&self, node: Node) -> Result<Stmt, ReadError> {
        Ok(Stmt::block(self.read_block_stmts(node)?))
    }

    fn read_block_stmts(&self, node: Node) -> Result<Vec<Stmt>, ReadError> {
        let mut stmts = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                if let Some(stmt) = self.read_stmt(child)? {
                    stmts.push(stmt);
                }
            }
        }

        Ok(stmts)
    }

    fn read_expr(&self, node: Node) -> Result<Expr, ReadError> {
        match node.kind() {
            // Literals
            "integer" | "float" => {
                let text = self.node_text(node);
                let num: f64 = text.parse().unwrap_or(0.0);
                Ok(Expr::number(num))
            }

            "string" | "concatenated_string" => {
                let text = self.node_text(node);
                // Remove quotes
                let inner = text
                    .trim_start_matches(|c| c == '"' || c == '\'')
                    .trim_start_matches("f\"")
                    .trim_start_matches("f'")
                    .trim_start_matches("r\"")
                    .trim_start_matches("r'")
                    .trim_end_matches(|c| c == '"' || c == '\'');
                Ok(Expr::string(inner))
            }

            "true" => Ok(Expr::bool(true)),
            "false" => Ok(Expr::bool(false)),
            "none" => Ok(Expr::null()),

            // Identifiers
            "identifier" => Ok(Expr::ident(self.node_text(node))),

            // Binary operations
            "binary_operator" => self.read_binary_operator(node),
            "comparison_operator" => self.read_comparison_operator(node),
            "boolean_operator" => self.read_boolean_operator(node),

            // Unary
            "unary_operator" => self.read_unary_operator(node),
            "not_operator" => {
                let arg = node
                    .child_by_field_name("argument")
                    .ok_or_else(|| ReadError::Parse("not_operator missing argument".into()))?;
                Ok(Expr::unary(UnaryOp::Not, self.read_expr(arg)?))
            }

            // Calls
            "call" => self.read_call(node),

            // Member access
            "attribute" => self.read_attribute(node),

            // Subscript
            "subscript" => self.read_subscript(node),

            // List/Dict/Tuple
            "list" => self.read_list(node),
            "dictionary" => self.read_dictionary(node),
            "tuple" => self.read_tuple(node),

            // Parenthesized
            "parenthesized_expression" => {
                let inner = node.child(1).ok_or_else(|| {
                    ReadError::Parse("parenthesized_expression missing inner".into())
                })?;
                self.read_expr(inner)
            }

            // Conditional expression (ternary)
            "conditional_expression" => self.read_conditional_expression(node),

            // Lambda
            "lambda" => self.read_lambda(node),

            // Assignment expression (walrus operator :=)
            "named_expression" => {
                let name = node
                    .child_by_field_name("name")
                    .ok_or_else(|| ReadError::Parse("named_expression missing name".into()))?;
                let value = node
                    .child_by_field_name("value")
                    .ok_or_else(|| ReadError::Parse("named_expression missing value".into()))?;
                Ok(Expr::assign(
                    Expr::ident(self.node_text(name)),
                    self.read_expr(value)?,
                ))
            }

            _ => {
                // Debug: print unknown node kind
                // eprintln!("Unknown Python expression kind: {}", node.kind());
                Err(ReadError::Parse(format!(
                    "unsupported expression: {}",
                    node.kind()
                )))
            }
        }
    }

    fn read_binary_operator(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("binary_operator missing left".into()))?;

        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("binary_operator missing right".into()))?;

        let op_node = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("binary_operator missing operator".into()))?;

        let op = match self.node_text(op_node) {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            "*" => BinaryOp::Mul,
            "/" | "//" => BinaryOp::Div,
            "%" => BinaryOp::Mod,
            _ => {
                return Err(ReadError::Parse(format!(
                    "unknown binary op: {}",
                    self.node_text(op_node)
                )));
            }
        };

        Ok(Expr::binary(
            self.read_expr(left)?,
            op,
            self.read_expr(right)?,
        ))
    }

    fn read_comparison_operator(&self, node: Node) -> Result<Expr, ReadError> {
        // Python comparison: a < b < c is chained, but we'll simplify to binary
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        if children.len() < 3 {
            return Err(ReadError::Parse(
                "comparison needs at least 3 children".into(),
            ));
        }

        let left = self.read_expr(children[0])?;
        let op_text = self.node_text(children[1]);
        let right = self.read_expr(children[2])?;

        let op = match op_text {
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            ">=" => BinaryOp::Ge,
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            _ => {
                return Err(ReadError::Parse(format!(
                    "unknown comparison op: {}",
                    op_text
                )));
            }
        };

        // Handle chained comparisons: a < b < c becomes (a < b) and (b < c)
        if children.len() > 3 {
            let mut result = Expr::binary(left, op, right.clone());
            let mut prev_right = right;

            for i in (3..children.len()).step_by(2) {
                if i + 1 < children.len() {
                    let next_op_text = self.node_text(children[i]);
                    let next_right = self.read_expr(children[i + 1])?;

                    let next_op = match next_op_text {
                        "<" => BinaryOp::Lt,
                        "<=" => BinaryOp::Le,
                        ">" => BinaryOp::Gt,
                        ">=" => BinaryOp::Ge,
                        "==" => BinaryOp::Eq,
                        "!=" => BinaryOp::Ne,
                        _ => continue,
                    };

                    let next_cmp = Expr::binary(prev_right, next_op, next_right.clone());
                    result = Expr::binary(result, BinaryOp::And, next_cmp);
                    prev_right = next_right;
                }
            }
            Ok(result)
        } else {
            Ok(Expr::binary(left, op, right))
        }
    }

    fn read_boolean_operator(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("boolean_operator missing left".into()))?;

        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("boolean_operator missing right".into()))?;

        let op_node = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("boolean_operator missing operator".into()))?;

        let op = match self.node_text(op_node) {
            "and" => BinaryOp::And,
            "or" => BinaryOp::Or,
            _ => {
                return Err(ReadError::Parse(format!(
                    "unknown boolean op: {}",
                    self.node_text(op_node)
                )));
            }
        };

        Ok(Expr::binary(
            self.read_expr(left)?,
            op,
            self.read_expr(right)?,
        ))
    }

    fn read_unary_operator(&self, node: Node) -> Result<Expr, ReadError> {
        let op_node = node
            .child_by_field_name("operator")
            .ok_or_else(|| ReadError::Parse("unary_operator missing operator".into()))?;

        let arg = node
            .child_by_field_name("argument")
            .ok_or_else(|| ReadError::Parse("unary_operator missing argument".into()))?;

        let op = match self.node_text(op_node) {
            "-" => UnaryOp::Neg,
            "+" => return self.read_expr(arg), // Unary + is no-op
            "~" => {
                return Err(ReadError::Parse(
                    "bitwise not (~) not supported in IR".into(),
                ));
            }
            _ => {
                return Err(ReadError::Parse(format!(
                    "unknown unary op: {}",
                    self.node_text(op_node)
                )));
            }
        };

        Ok(Expr::unary(op, self.read_expr(arg)?))
    }

    fn read_call(&self, node: Node) -> Result<Expr, ReadError> {
        let function = node
            .child_by_field_name("function")
            .ok_or_else(|| ReadError::Parse("call missing function".into()))?;

        let arguments = node.child_by_field_name("arguments");

        let callee = self.read_expr(function)?;
        let args = arguments
            .map(|a| self.read_arguments(a))
            .transpose()?
            .unwrap_or_default();

        Ok(Expr::call(callee, args))
    }

    fn read_arguments(&self, node: Node) -> Result<Vec<Expr>, ReadError> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() && child.kind() != "(" && child.kind() != ")" {
                // Skip keyword arguments for now (just get positional)
                if child.kind() != "keyword_argument" {
                    args.push(self.read_expr(child)?);
                }
            }
        }

        Ok(args)
    }

    fn read_attribute(&self, node: Node) -> Result<Expr, ReadError> {
        let object = node
            .child_by_field_name("object")
            .ok_or_else(|| ReadError::Parse("attribute missing object".into()))?;

        let attribute = node
            .child_by_field_name("attribute")
            .ok_or_else(|| ReadError::Parse("attribute missing attribute".into()))?;

        let obj_expr = self.read_expr(object)?;
        let prop = self.node_text(attribute);

        Ok(Expr::member(obj_expr, prop))
    }

    fn read_subscript(&self, node: Node) -> Result<Expr, ReadError> {
        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| ReadError::Parse("subscript missing value".into()))?;

        let subscript = node
            .child_by_field_name("subscript")
            .ok_or_else(|| ReadError::Parse("subscript missing subscript".into()))?;

        let obj_expr = self.read_expr(value)?;
        let idx_expr = self.read_expr(subscript)?;

        Ok(Expr::index(obj_expr, idx_expr))
    }

    fn read_list(&self, node: Node) -> Result<Expr, ReadError> {
        let mut items = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                items.push(self.read_expr(child)?);
            }
        }

        Ok(Expr::array(items))
    }

    fn read_dictionary(&self, node: Node) -> Result<Expr, ReadError> {
        let mut pairs = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "pair" {
                let key = child.child_by_field_name("key");
                let value = child.child_by_field_name("value");

                if let (Some(k), Some(v)) = (key, value) {
                    let key_text = self.node_text(k);
                    // Remove quotes from string keys
                    let key_str = key_text.trim_matches('"').trim_matches('\'').to_string();
                    pairs.push((key_str, self.read_expr(v)?));
                }
            }
        }

        Ok(Expr::object(pairs))
    }

    fn read_tuple(&self, node: Node) -> Result<Expr, ReadError> {
        // Treat tuples as arrays (closest equivalent)
        let mut items = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                items.push(self.read_expr(child)?);
            }
        }

        Ok(Expr::array(items))
    }

    fn read_conditional_expression(&self, node: Node) -> Result<Expr, ReadError> {
        // Python ternary: value_if_true if condition else value_if_false
        // But tree-sitter field names may differ
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|c| c.is_named())
            .collect();

        if children.len() >= 3 {
            // Pattern: consequent if test else alternate
            let consequent = self.read_expr(children[0])?;
            let test = self.read_expr(children[1])?;
            let alternate = self.read_expr(children[2])?;

            Ok(Expr::conditional(test, consequent, alternate))
        } else {
            Err(ReadError::Parse(
                "conditional_expression needs 3 parts".into(),
            ))
        }
    }

    fn read_lambda(&self, node: Node) -> Result<Expr, ReadError> {
        let params = node.child_by_field_name("parameters");
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("lambda missing body".into()))?;

        let fn_params = params
            .map(|p| self.read_lambda_parameters(p))
            .transpose()?
            .unwrap_or_default();

        let body_expr = self.read_expr(body)?;

        // Lambda body is an expression, wrap in return
        let fn_body = vec![Stmt::return_stmt(Some(body_expr))];

        Ok(Expr::Function(Box::new(Function::anonymous(
            fn_params, fn_body,
        ))))
    }

    fn read_lambda_parameters(&self, node: Node) -> Result<Vec<String>, ReadError> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                params.push(self.node_text(child).to_string());
            }
        }

        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_assignment() {
        let ir = read_python("x = 42").unwrap();
        assert_eq!(ir.body.len(), 1);
        match &ir.body[0] {
            Stmt::Let { name, .. } => assert_eq!(name, "x"),
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn test_binary_expr() {
        let ir = read_python("result = 1 + 2 * 3").unwrap();
        assert_eq!(ir.body.len(), 1);
    }

    #[test]
    fn test_function_call() {
        let ir = read_python("print(\"hello\", 42)").unwrap();
        assert_eq!(ir.body.len(), 1);
        match &ir.body[0] {
            Stmt::Expr(Expr::Call { callee, args }) => {
                assert!(matches!(callee.as_ref(), Expr::Ident(n) if n == "print"));
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn test_function_declaration() {
        let ir = read_python("def add(a, b):\n    return a + b").unwrap();
        assert_eq!(ir.body.len(), 1);
        match &ir.body[0] {
            Stmt::Function(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params, vec!["a", "b"]);
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_if_statement() {
        let ir = read_python("if x > 0:\n    print(x)").unwrap();
        assert_eq!(ir.body.len(), 1);
        assert!(matches!(&ir.body[0], Stmt::If { .. }));
    }

    #[test]
    fn test_for_loop() {
        let ir = read_python("for i in items:\n    print(i)").unwrap();
        assert_eq!(ir.body.len(), 1);
        match &ir.body[0] {
            Stmt::ForIn { variable, .. } => assert_eq!(variable, "i"),
            _ => panic!("expected ForIn"),
        }
    }

    #[test]
    fn test_list_literal() {
        let ir = read_python("arr = [1, 2, 3]").unwrap();
        assert_eq!(ir.body.len(), 1);
    }

    #[test]
    fn test_dict_literal() {
        let ir = read_python("obj = {\"x\": 1, \"y\": 2}").unwrap();
        assert_eq!(ir.body.len(), 1);
    }
}
