//! Convert AST to S-expressions.

use crate::ir::*;
use serde_json::{Value, json};

/// Convert a program to S-expression format.
pub fn to_sexpr(program: &Program) -> Value {
    if program.body.len() == 1 {
        stmt_to_sexpr(&program.body[0])
    } else {
        let mut arr = vec![json!("std.seq")];
        arr.extend(program.body.iter().map(stmt_to_sexpr));
        Value::Array(arr)
    }
}

fn stmt_to_sexpr(stmt: &Stmt) -> Value {
    match stmt {
        Stmt::Expr(expr) => expr_to_sexpr(expr),

        Stmt::Let { name, init, .. } => {
            if let Some(init) = init {
                json!(["std.let", name, expr_to_sexpr(init)])
            } else {
                json!(["std.let", name, null])
            }
        }

        Stmt::Block(stmts) => {
            if stmts.is_empty() {
                Value::Null
            } else if stmts.len() == 1 {
                stmt_to_sexpr(&stmts[0])
            } else {
                let mut arr = vec![json!("std.seq")];
                arr.extend(stmts.iter().map(stmt_to_sexpr));
                Value::Array(arr)
            }
        }

        Stmt::If {
            test,
            consequent,
            alternate,
        } => {
            let mut arr = vec![
                json!("std.if"),
                expr_to_sexpr(test),
                stmt_to_sexpr(consequent),
            ];
            if let Some(alt) = alternate {
                arr.push(stmt_to_sexpr(alt));
            }
            Value::Array(arr)
        }

        Stmt::While { test, body } => {
            json!(["std.while", expr_to_sexpr(test), stmt_to_sexpr(body)])
        }

        Stmt::For {
            init,
            test,
            update,
            body,
        } => {
            // Convert to: std.seq(init, std.while(test, std.seq(body, update)))
            let body_sexpr = stmt_to_sexpr(body);
            let while_body = if let Some(upd) = update {
                json!(["std.seq", body_sexpr, expr_to_sexpr(upd)])
            } else {
                body_sexpr
            };

            let test_sexpr = test
                .as_ref()
                .map(expr_to_sexpr)
                .unwrap_or(Value::Bool(true));
            let while_loop = json!(["std.while", test_sexpr, while_body]);

            if let Some(init) = init {
                json!(["std.seq", stmt_to_sexpr(init), while_loop])
            } else {
                while_loop
            }
        }

        Stmt::ForIn {
            variable,
            iterable,
            body,
        } => {
            json!([
                "std.for",
                variable,
                expr_to_sexpr(iterable),
                stmt_to_sexpr(body)
            ])
        }

        Stmt::Return(expr) => {
            if let Some(e) = expr {
                json!(["std.return", expr_to_sexpr(e)])
            } else {
                json!(["std.return"])
            }
        }

        Stmt::Break => json!(["std.break"]),
        Stmt::Continue => json!(["std.continue"]),

        Stmt::TryCatch {
            body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            let mut parts: Vec<Value> = vec![json!("std.try"), stmt_to_sexpr(body)];
            if let Some(cb) = catch_body {
                let param = catch_param
                    .as_deref()
                    .map(|s| json!(s))
                    .unwrap_or(Value::Null);
                parts.push(json!(["catch", param, stmt_to_sexpr(cb)]));
            }
            if let Some(fb) = finally_body {
                parts.push(json!(["finally", stmt_to_sexpr(fb)]));
            }
            Value::Array(parts)
        }

        Stmt::Function(f) => function_to_sexpr(f),
    }
}

fn expr_to_sexpr(expr: &Expr) -> Value {
    match expr {
        Expr::Literal(lit) => literal_to_sexpr(lit),

        Expr::Ident(name) => json!(["std.var", name]),

        Expr::Binary { left, op, right } => {
            let opcode = binary_op_to_opcode(*op);
            json!([opcode, expr_to_sexpr(left), expr_to_sexpr(right)])
        }

        Expr::Unary { op, expr } => {
            let opcode = unary_op_to_opcode(*op);
            json!([opcode, expr_to_sexpr(expr)])
        }

        Expr::Call { callee, args } => {
            // Try to get a simple function name for the opcode
            if let Some(name) = get_call_name(callee) {
                let mut arr = vec![json!(name)];
                arr.extend(args.iter().map(expr_to_sexpr));
                Value::Array(arr)
            } else {
                // Complex callee - use std.apply
                let mut arr = vec![json!("std.apply"), expr_to_sexpr(callee)];
                arr.extend(args.iter().map(expr_to_sexpr));
                Value::Array(arr)
            }
        }

        Expr::Member {
            object,
            property,
            computed,
        } => {
            if *computed {
                // obj[expr] - could be list or object access
                json!(["obj.get", expr_to_sexpr(object), expr_to_sexpr(property)])
            } else if let Expr::Literal(Literal::String(prop)) = property.as_ref() {
                json!(["obj.get", expr_to_sexpr(object), prop])
            } else {
                json!(["obj.get", expr_to_sexpr(object), expr_to_sexpr(property)])
            }
        }

        Expr::Array(items) => {
            let mut arr = vec![json!("list.new")];
            arr.extend(items.iter().map(expr_to_sexpr));
            Value::Array(arr)
        }

        Expr::Object(pairs) => {
            let mut arr = vec![json!("obj.new")];
            for (key, value) in pairs {
                arr.push(json!([key, expr_to_sexpr(value)]));
            }
            Value::Array(arr)
        }

        Expr::Function(f) => function_to_sexpr(f),

        Expr::Conditional {
            test,
            consequent,
            alternate,
        } => {
            json!([
                "std.if",
                expr_to_sexpr(test),
                expr_to_sexpr(consequent),
                expr_to_sexpr(alternate)
            ])
        }

        Expr::Assign { target, value } => {
            match target.as_ref() {
                Expr::Ident(name) => {
                    json!(["std.set", name, expr_to_sexpr(value)])
                }
                Expr::Member {
                    object, property, ..
                } => {
                    if let Expr::Literal(Literal::String(prop)) = property.as_ref() {
                        json!(["obj.set", expr_to_sexpr(object), prop, expr_to_sexpr(value)])
                    } else {
                        json!([
                            "obj.set",
                            expr_to_sexpr(object),
                            expr_to_sexpr(property),
                            expr_to_sexpr(value)
                        ])
                    }
                }
                _ => {
                    // Fallback for complex assignment targets
                    json!(["std.set", expr_to_sexpr(target), expr_to_sexpr(value)])
                }
            }
        }
    }
}

fn literal_to_sexpr(lit: &Literal) -> Value {
    match lit {
        Literal::Null => Value::Null,
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Number(n) => json!(n),
        Literal::String(s) => Value::String(s.clone()),
    }
}

fn function_to_sexpr(f: &Function) -> Value {
    let params: Vec<Value> = f.params.iter().map(|p| json!(p)).collect();
    let body = if f.body.len() == 1 {
        stmt_to_sexpr(&f.body[0])
    } else {
        let mut arr = vec![json!("std.seq")];
        arr.extend(f.body.iter().map(stmt_to_sexpr));
        Value::Array(arr)
    };

    if f.name.is_empty() {
        json!(["std.lambda", params, body])
    } else {
        json!(["std.fn", f.name, params, body])
    }
}

fn binary_op_to_opcode(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "math.add",
        BinaryOp::Sub => "math.sub",
        BinaryOp::Mul => "math.mul",
        BinaryOp::Div => "math.div",
        BinaryOp::Mod => "math.mod",
        BinaryOp::Eq => "bool.eq",
        BinaryOp::Ne => "bool.neq",
        BinaryOp::Lt => "bool.lt",
        BinaryOp::Le => "bool.lte",
        BinaryOp::Gt => "bool.gt",
        BinaryOp::Ge => "bool.gte",
        BinaryOp::And => "bool.and",
        BinaryOp::Or => "bool.or",
        BinaryOp::Concat => "str.concat",
    }
}

fn unary_op_to_opcode(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "math.neg",
        UnaryOp::Not => "bool.not",
    }
}

fn get_call_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Member {
            object,
            property,
            computed: false,
        } => {
            if let (Some(obj_name), Expr::Literal(Literal::String(prop))) =
                (get_call_name(object), property.as_ref())
            {
                Some(format!("{}.{}", obj_name, prop))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_let() {
        let program = Program::new(vec![Stmt::const_decl("x", Expr::number(42))]);
        let sexpr = to_sexpr(&program);
        assert_eq!(sexpr, json!(["std.let", "x", 42.0]));
    }

    #[test]
    fn test_binary_expr() {
        let program = Program::new(vec![Stmt::expr(Expr::binary(
            Expr::number(1),
            BinaryOp::Add,
            Expr::number(2),
        ))]);
        let sexpr = to_sexpr(&program);
        assert_eq!(sexpr, json!(["math.add", 1.0, 2.0]));
    }

    #[test]
    fn test_function_call() {
        let program = Program::new(vec![Stmt::expr(Expr::call(
            Expr::member(Expr::ident("console"), "log"),
            vec![Expr::string("hello")],
        ))]);
        let sexpr = to_sexpr(&program);
        assert_eq!(sexpr, json!(["console.log", "hello"]));
    }
}
