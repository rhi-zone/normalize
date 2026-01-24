//! Tree-sitter based Lua reader.

use crate::ir::*;
use crate::traits::{ReadError, Reader};
use tree_sitter::{Node, Parser, Tree};

/// Static instance of the Lua reader for registry.
pub static LUA_READER: LuaReader = LuaReader;

/// Lua reader using tree-sitter.
pub struct LuaReader;

impl Reader for LuaReader {
    fn language(&self) -> &'static str {
        "lua"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn read(&self, source: &str) -> Result<Program, ReadError> {
        read_lua(source)
    }
}

/// Parse Lua source into surface-syntax IR.
pub fn read_lua(source: &str) -> Result<Program, ReadError> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_lua::language().into())
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

            // Variable declarations (Lua grammar: variable_declaration contains local + assignment)
            "variable_declaration" => self.read_local_variable_declaration(node).map(Some),

            // Assignment (non-local)
            "assignment_statement" => self.read_assignment_statement(node).map(Some),

            // Control flow
            "if_statement" => self.read_if_statement(node).map(Some),
            "while_statement" => self.read_while_statement(node).map(Some),
            "repeat_statement" => self.read_repeat_statement(node).map(Some),
            "for_statement" => self.read_for_statement(node).map(Some),
            "do_statement" => self.read_do_statement(node).map(Some),

            // Jumps
            "return_statement" => self.read_return_statement(node).map(Some),
            "break_statement" => Ok(Some(Stmt::break_stmt())),

            // Functions
            "function_declaration" => self.read_function_declaration(node).map(Some),
            "local_function_declaration" => self.read_local_function_declaration(node).map(Some),

            // Function call as statement
            "function_call" => {
                let expr = self.read_function_call(node)?;
                Ok(Some(Stmt::expr(expr)))
            }

            // Expression statements
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
            "nil" => Ok(Expr::null()),

            // Identifiers
            "identifier" => Ok(Expr::ident(self.node_text(node))),

            // Expressions
            "binary_expression" => self.read_binary_expr(node),
            "unary_expression" => self.read_unary_expr(node),
            "parenthesized_expression" => self.read_parenthesized(node),
            "function_call" => self.read_function_call(node),
            "dot_index_expression" => self.read_dot_index(node),
            "bracket_index_expression" => self.read_bracket_index(node),
            "table_constructor" => self.read_table(node),
            "function_definition" => self.read_function_expr(node),

            // Method calls (obj:method(args))
            "method_index_expression" => self.read_method_index(node),

            kind => Err(ReadError::Unsupported(format!(
                "expression type '{}': {}",
                kind,
                self.node_text(node)
            ))),
        }
    }

    fn read_number(&self, node: Node) -> Result<Expr, ReadError> {
        let text = self.node_text(node);
        let value: f64 = text
            .parse()
            .map_err(|_| ReadError::Parse(format!("invalid number: {}", text)))?;
        Ok(Expr::number(value))
    }

    fn read_string(&self, node: Node) -> Result<Expr, ReadError> {
        let text = self.node_text(node);
        // Handle different string formats
        let inner = if text.starts_with("[[") {
            // Long string [[...]]
            text.strip_prefix("[[")
                .and_then(|s| s.strip_suffix("]]"))
                .unwrap_or(text)
        } else if text.starts_with("[=[") {
            // Long string with equals [=[...]=]
            let start = text.find('[').unwrap() + 1;
            let end = text.rfind(']').unwrap();
            let equals_count = text[start..].chars().take_while(|c| *c == '=').count();
            let actual_start = start + equals_count + 1;
            let actual_end = end - equals_count;
            &text[actual_start..actual_end]
        } else if text.starts_with('"') || text.starts_with('\'') {
            &text[1..text.len() - 1]
        } else {
            text
        };

        // Handle escapes
        let unescaped = inner
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\'", "'")
            .replace("\\\\", "\\");

        Ok(Expr::string(unescaped))
    }

    fn read_binary_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| ReadError::Parse("binary_expression missing left".into()))?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| ReadError::Parse("binary_expression missing right".into()))?;

        // Find the operator - it's an anonymous child between left and right
        let mut operator_text = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                let text = self.node_text(child).trim();
                if !text.is_empty() {
                    operator_text = Some(text);
                    break;
                }
            }
        }

        let op_text = operator_text
            .ok_or_else(|| ReadError::Parse("binary_expression missing operator".into()))?;

        let left_expr = self.read_expr(left)?;
        let right_expr = self.read_expr(right)?;

        let op = match op_text {
            // Arithmetic
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            "%" => BinaryOp::Mod,
            "//" => BinaryOp::Div, // Integer division maps to div

            // String concatenation
            ".." => BinaryOp::Concat,

            // Comparison
            "==" => BinaryOp::Eq,
            "~=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            ">" => BinaryOp::Gt,
            "<=" => BinaryOp::Le,
            ">=" => BinaryOp::Ge,

            // Logical
            "and" => BinaryOp::And,
            "or" => BinaryOp::Or,

            // Power
            "^" => {
                return Ok(Expr::call(
                    Expr::member(Expr::ident("math"), "pow"),
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
        // Find operator and operand
        let mut operator_text = None;
        let mut operand_node = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                let text = self.node_text(child).trim();
                if !text.is_empty() && operator_text.is_none() {
                    operator_text = Some(text);
                }
            } else if operand_node.is_none() {
                operand_node = Some(child);
            }
        }

        let op_text = operator_text
            .ok_or_else(|| ReadError::Parse("unary_expression missing operator".into()))?;
        let operand = operand_node
            .ok_or_else(|| ReadError::Parse("unary_expression missing operand".into()))?;

        let arg_expr = self.read_expr(operand)?;

        let op = match op_text {
            "not" => UnaryOp::Not,
            "-" => UnaryOp::Neg,
            "#" => {
                // Length operator -> call to table.len or string.len
                return Ok(Expr::call(Expr::ident("len"), vec![arg_expr]));
            }
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

    fn read_function_call(&self, node: Node) -> Result<Expr, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("function_call missing name".into()))?;
        let arguments = node.child_by_field_name("arguments");

        let callee = self.read_expr(name)?;

        // Parse arguments
        let args = if let Some(args_node) = arguments {
            self.read_arguments(args_node)?
        } else {
            // Lua allows: f"string" and f{table} as shorthand
            // Look for string or table following the name
            let mut args = Vec::new();
            let mut cursor = node.walk();
            let mut past_name = false;
            for child in node.children(&mut cursor) {
                if child.id() == name.id() {
                    past_name = true;
                    continue;
                }
                if past_name && child.is_named() {
                    args.push(self.read_expr(child)?);
                    break;
                }
            }
            args
        };

        Ok(Expr::call(callee, args))
    }

    fn read_arguments(&self, node: Node) -> Result<Vec<Expr>, ReadError> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.is_named() {
                args.push(self.read_expr(child)?);
            }
        }

        Ok(args)
    }

    fn read_dot_index(&self, node: Node) -> Result<Expr, ReadError> {
        let table = node
            .child_by_field_name("table")
            .ok_or_else(|| ReadError::Parse("dot_index_expression missing table".into()))?;
        let field = node
            .child_by_field_name("field")
            .ok_or_else(|| ReadError::Parse("dot_index_expression missing field".into()))?;

        let table_expr = self.read_expr(table)?;
        let field_name = self.node_text(field);

        Ok(Expr::member(table_expr, field_name))
    }

    fn read_bracket_index(&self, node: Node) -> Result<Expr, ReadError> {
        let table = node
            .child_by_field_name("table")
            .ok_or_else(|| ReadError::Parse("bracket_index_expression missing table".into()))?;
        let index = node
            .child_by_field_name("field")
            .ok_or_else(|| ReadError::Parse("bracket_index_expression missing field".into()))?;

        let table_expr = self.read_expr(table)?;
        let index_expr = self.read_expr(index)?;

        Ok(Expr::index(table_expr, index_expr))
    }

    fn read_method_index(&self, node: Node) -> Result<Expr, ReadError> {
        // obj:method -> obj.method with implicit self
        let table = node
            .child_by_field_name("table")
            .ok_or_else(|| ReadError::Parse("method_index_expression missing table".into()))?;
        let method = node
            .child_by_field_name("method")
            .ok_or_else(|| ReadError::Parse("method_index_expression missing method".into()))?;

        let table_expr = self.read_expr(table)?;
        let method_name = self.node_text(method);

        Ok(Expr::member(table_expr, method_name))
    }

    fn read_table(&self, node: Node) -> Result<Expr, ReadError> {
        let mut pairs: Vec<(String, Expr)> = Vec::new();
        let mut array_index = 1;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "field" => {
                    // Named field: key = value
                    let name = child.child_by_field_name("name");
                    let value = child
                        .child_by_field_name("value")
                        .ok_or_else(|| ReadError::Parse("field missing value".into()))?;

                    let key = if let Some(name_node) = name {
                        self.node_text(name_node).to_string()
                    } else {
                        // Implicit numeric key for array-style entries
                        let k = array_index.to_string();
                        array_index += 1;
                        k
                    };

                    pairs.push((key, self.read_expr(value)?));
                }
                _ if child.is_named() => {
                    // Array-style entry without explicit key
                    let key = array_index.to_string();
                    array_index += 1;
                    pairs.push((key, self.read_expr(child)?));
                }
                _ => {}
            }
        }

        Ok(Expr::object(pairs))
    }

    fn read_function_expr(&self, node: Node) -> Result<Expr, ReadError> {
        let mut param_names = Vec::new();
        if let Some(params) = node.child_by_field_name("parameters") {
            self.collect_params(params, &mut param_names);
        }

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function_definition missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        Ok(Expr::Function(Box::new(Function::anonymous(
            param_names,
            body,
        ))))
    }

    fn collect_params(&self, params: Node, param_names: &mut Vec<String>) {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() == "identifier" {
                param_names.push(self.node_text(child).to_string());
            } else if child.kind() == "vararg_expression" {
                param_names.push("...".to_string());
            }
        }
    }

    fn read_local_variable_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        // Structure: variable_declaration -> [local, assignment_statement]
        // The assignment_statement contains: variable_list = expression_list
        let mut names = Vec::new();
        let mut values = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment_statement" {
                // Parse the assignment statement within
                let mut assign_cursor = child.walk();
                for assign_child in child.children(&mut assign_cursor) {
                    if assign_child.kind() == "variable_list" {
                        let mut inner_cursor = assign_child.walk();
                        for name_node in assign_child.children(&mut inner_cursor) {
                            if name_node.kind() == "identifier" {
                                names.push(self.node_text(name_node).to_string());
                            }
                        }
                    } else if assign_child.kind() == "expression_list" {
                        let mut inner_cursor = assign_child.walk();
                        for val_node in assign_child.children(&mut inner_cursor) {
                            if val_node.is_named() {
                                values.push(self.read_expr(val_node)?);
                            }
                        }
                    }
                }
            } else if child.kind() == "variable_list" {
                // Direct variable_list (some grammars)
                let mut inner_cursor = child.walk();
                for name_node in child.children(&mut inner_cursor) {
                    if name_node.kind() == "identifier" {
                        names.push(self.node_text(name_node).to_string());
                    }
                }
            } else if child.kind() == "expression_list" {
                let mut inner_cursor = child.walk();
                for val_node in child.children(&mut inner_cursor) {
                    if val_node.is_named() {
                        values.push(self.read_expr(val_node)?);
                    }
                }
            }
        }

        // Handle single variable case
        if names.len() == 1 {
            let init = values.into_iter().next();
            return Ok(Stmt::Let {
                name: names.remove(0),
                init,
                mutable: true,
            });
        }

        // Multi-variable declaration: local a, b = 1, 2
        let mut stmts = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let init = values.get(i).cloned();
            stmts.push(Stmt::Let {
                name,
                init,
                mutable: true,
            });
        }

        Ok(Stmt::block(stmts))
    }

    fn read_assignment_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let mut targets = Vec::new();
        let mut values = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_list" {
                let mut inner_cursor = child.walk();
                for target_node in child.children(&mut inner_cursor) {
                    if target_node.is_named() {
                        targets.push(self.read_expr(target_node)?);
                    }
                }
            } else if child.kind() == "expression_list" {
                let mut inner_cursor = child.walk();
                for val_node in child.children(&mut inner_cursor) {
                    if val_node.is_named() {
                        values.push(self.read_expr(val_node)?);
                    }
                }
            }
        }

        // Single assignment
        if targets.len() == 1 && values.len() == 1 {
            return Ok(Stmt::expr(Expr::assign(
                targets.remove(0),
                values.remove(0),
            )));
        }

        // Multi-assignment: a, b = 1, 2
        let mut stmts = Vec::new();
        for (target, value) in targets.into_iter().zip(values.into_iter()) {
            stmts.push(Stmt::expr(Expr::assign(target, value)));
        }

        Ok(Stmt::block(stmts))
    }

    fn read_if_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("if_statement missing condition".into()))?;
        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("if_statement missing consequence".into()))?;

        let cond_expr = self.read_expr(condition)?;
        let then_stmts = self.read_block_stmts(consequence)?;
        let then_stmt = Stmt::block(then_stmts);

        // Handle elseif and else
        let mut else_stmt: Option<Stmt> = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "elseif_statement" => {
                    let elseif = self.read_elseif_statement(child)?;
                    else_stmt = Some(elseif);
                }
                "else_statement" => {
                    let else_body = child
                        .child_by_field_name("body")
                        .ok_or_else(|| ReadError::Parse("else_statement missing body".into()))?;
                    let stmts = self.read_block_stmts(else_body)?;
                    else_stmt = Some(Stmt::block(stmts));
                }
                _ => {}
            }
        }

        Ok(Stmt::if_stmt(cond_expr, then_stmt, else_stmt))
    }

    fn read_elseif_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("elseif_statement missing condition".into()))?;
        let consequence = node
            .child_by_field_name("consequence")
            .ok_or_else(|| ReadError::Parse("elseif_statement missing consequence".into()))?;

        let cond_expr = self.read_expr(condition)?;
        let then_stmts = self.read_block_stmts(consequence)?;
        let then_stmt = Stmt::block(then_stmts);

        // Check for more elseif or else
        let mut else_stmt: Option<Stmt> = None;
        if let Some(sibling) = node.next_sibling() {
            match sibling.kind() {
                "elseif_statement" => {
                    else_stmt = Some(self.read_elseif_statement(sibling)?);
                }
                "else_statement" => {
                    let else_body = sibling
                        .child_by_field_name("body")
                        .ok_or_else(|| ReadError::Parse("else_statement missing body".into()))?;
                    let stmts = self.read_block_stmts(else_body)?;
                    else_stmt = Some(Stmt::block(stmts));
                }
                _ => {}
            }
        }

        Ok(Stmt::if_stmt(cond_expr, then_stmt, else_stmt))
    }

    fn read_while_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("while_statement missing condition".into()))?;
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("while_statement missing body".into()))?;

        let cond_expr = self.read_expr(condition)?;
        let body_stmts = self.read_block_stmts(body)?;

        Ok(Stmt::while_loop(cond_expr, Stmt::block(body_stmts)))
    }

    fn read_repeat_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        // repeat ... until condition -> equivalent to do { ... } while (!condition)
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("repeat_statement missing body".into()))?;
        let condition = node
            .child_by_field_name("condition")
            .ok_or_else(|| ReadError::Parse("repeat_statement missing condition".into()))?;

        let body_stmts = self.read_block_stmts(body)?;
        let cond_expr = self.read_expr(condition)?;

        // Convert to: while true { body; if condition then break end }
        let break_if = Stmt::if_stmt(cond_expr, Stmt::break_stmt(), None);
        let mut loop_body = body_stmts;
        loop_body.push(break_if);

        Ok(Stmt::while_loop(Expr::bool(true), Stmt::block(loop_body)))
    }

    fn read_for_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        // Check if it's a numeric for or generic for
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "for_generic_clause" {
                return self.read_for_generic(node, child);
            } else if child.kind() == "for_numeric_clause" {
                return self.read_for_numeric(node, child);
            }
        }

        Err(ReadError::Parse("for_statement missing clause".into()))
    }

    fn read_for_numeric(&self, node: Node, clause: Node) -> Result<Stmt, ReadError> {
        // for i = start, stop[, step] do ... end
        let name = clause
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("for_numeric_clause missing name".into()))?;
        let start = clause
            .child_by_field_name("start")
            .ok_or_else(|| ReadError::Parse("for_numeric_clause missing start".into()))?;
        let finish = clause
            .child_by_field_name("end")
            .ok_or_else(|| ReadError::Parse("for_numeric_clause missing end".into()))?;

        let var_name = self.node_text(name).to_string();
        let start_expr = self.read_expr(start)?;
        let finish_expr = self.read_expr(finish)?;

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("for_statement missing body".into()))?;
        let body_stmts = self.read_block_stmts(body_node)?;

        // Convert to: for (init; test; update) { body }
        let init = Stmt::let_decl(var_name.clone(), Some(start_expr));
        let test = Expr::binary(
            Expr::ident(var_name.clone()),
            BinaryOp::Le,
            finish_expr.clone(),
        );
        let update = Expr::assign(
            Expr::ident(var_name.clone()),
            Expr::binary(Expr::ident(var_name), BinaryOp::Add, Expr::number(1.0)),
        );

        Ok(Stmt::for_loop(
            Some(init),
            Some(test),
            Some(update),
            Stmt::block(body_stmts),
        ))
    }

    fn read_for_generic(&self, node: Node, clause: Node) -> Result<Stmt, ReadError> {
        // for k, v in pairs(t) do ... end
        let mut var_names = Vec::new();
        let mut iterator = None;

        let mut cursor = clause.walk();
        for child in clause.children(&mut cursor) {
            if child.kind() == "variable_list" {
                let mut inner_cursor = child.walk();
                for name_node in child.children(&mut inner_cursor) {
                    if name_node.kind() == "identifier" {
                        var_names.push(self.node_text(name_node).to_string());
                    }
                }
            } else if child.kind() == "expression_list" {
                let mut inner_cursor = child.walk();
                for expr_node in child.children(&mut inner_cursor) {
                    if expr_node.is_named() && iterator.is_none() {
                        iterator = Some(self.read_expr(expr_node)?);
                    }
                }
            }
        }

        let iter_expr = iterator
            .ok_or_else(|| ReadError::Parse("for_generic_clause missing iterator".into()))?;

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("for_statement missing body".into()))?;
        let body_stmts = self.read_block_stmts(body_node)?;

        // Use first variable as iteration variable
        let var_name = var_names
            .into_iter()
            .next()
            .unwrap_or_else(|| "_".to_string());

        Ok(Stmt::for_in(var_name, iter_expr, Stmt::block(body_stmts)))
    }

    fn read_do_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("do_statement missing body".into()))?;
        let stmts = self.read_block_stmts(body)?;
        Ok(Stmt::block(stmts))
    }

    fn read_return_statement(&self, node: Node) -> Result<Stmt, ReadError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "expression_list" {
                let mut inner_cursor = child.walk();
                for expr_node in child.children(&mut inner_cursor) {
                    if expr_node.is_named() {
                        // Return first value (multiple return values not directly supported in IR)
                        let value = self.read_expr(expr_node)?;
                        return Ok(Stmt::return_stmt(Some(value)));
                    }
                }
            } else if child.is_named() && child.kind() != "return" {
                let value = self.read_expr(child)?;
                return Ok(Stmt::return_stmt(Some(value)));
            }
        }
        Ok(Stmt::return_stmt(None))
    }

    fn read_function_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ReadError::Parse("function_declaration missing name".into()))?;

        let name_str = self.node_text(name).to_string();

        let mut param_names = Vec::new();
        if let Some(params) = node.child_by_field_name("parameters") {
            self.collect_params(params, &mut param_names);
        }

        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| ReadError::Parse("function_declaration missing body".into()))?;

        let body = self.read_block_stmts(body_node)?;

        Ok(Stmt::function(Function::new(name_str, param_names, body)))
    }

    fn read_local_function_declaration(&self, node: Node) -> Result<Stmt, ReadError> {
        // Same as function_declaration but marked as local
        self.read_function_declaration(node)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_assignment() {
        let program = read_lua("local x = 42").unwrap();
        assert_eq!(program.body.len(), 1);
        match &program.body[0] {
            Stmt::Let { name, init, .. } => {
                assert_eq!(name, "x");
                assert!(init.is_some());
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn test_binary_expr() {
        let program = read_lua("local x = 1 + 2").unwrap();
        match &program.body[0] {
            Stmt::Let {
                init: Some(Expr::Binary { op, .. }),
                ..
            } => {
                assert_eq!(op, &BinaryOp::Add);
            }
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_function_call() {
        let program = read_lua("print('hello')").unwrap();
        match &program.body[0] {
            Stmt::Expr(Expr::Call { callee, args }) => {
                assert_eq!(args.len(), 1);
                match callee.as_ref() {
                    Expr::Ident(name) => assert_eq!(name, "print"),
                    _ => panic!("expected Ident"),
                }
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn test_function_declaration() {
        let program = read_lua("function add(a, b) return a + b end").unwrap();
        match &program.body[0] {
            Stmt::Function(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params, vec!["a", "b"]);
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_if_statement() {
        let program = read_lua("if x > 0 then return 1 else return 0 end").unwrap();
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

    #[test]
    fn test_table_constructor() {
        let program = read_lua("local t = { a = 1, b = 2 }").unwrap();
        match &program.body[0] {
            Stmt::Let {
                init: Some(Expr::Object(pairs)),
                ..
            } => {
                assert_eq!(pairs.len(), 2);
            }
            _ => panic!("expected Object"),
        }
    }
}
