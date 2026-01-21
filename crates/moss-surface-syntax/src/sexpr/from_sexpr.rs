//! Convert S-expressions to AST.

use super::SExprError;
use crate::ir::*;
use serde_json::Value;

/// Convert S-expression to a program.
pub fn from_sexpr(sexpr: &Value) -> Result<Program, SExprError> {
    let stmt = value_to_stmt(sexpr)?;
    Ok(Program::new(vec![stmt]))
}

fn value_to_stmt(value: &Value) -> Result<Stmt, SExprError> {
    match value {
        // Literals become expression statements
        Value::Null => Ok(Stmt::expr(Expr::null())),
        Value::Bool(b) => Ok(Stmt::expr(Expr::bool(*b))),
        Value::Number(n) => Ok(Stmt::expr(Expr::number(n.as_f64().unwrap_or(0.0)))),
        Value::String(s) => Ok(Stmt::expr(Expr::string(s.clone()))),

        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(Stmt::expr(Expr::array(vec![])));
            }

            // First element should be the opcode
            let opcode = arr[0]
                .as_str()
                .ok_or_else(|| SExprError::ExpectedOpcode(format!("{:?}", arr[0])))?;

            let args = &arr[1..];
            parse_opcode(opcode, args)
        }

        Value::Object(_) => Err(SExprError::InvalidArgument(
            "unexpected object in S-expression".into(),
        )),
    }
}

fn value_to_expr(value: &Value) -> Result<Expr, SExprError> {
    match value {
        Value::Null => Ok(Expr::null()),
        Value::Bool(b) => Ok(Expr::bool(*b)),
        Value::Number(n) => Ok(Expr::number(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Ok(Expr::string(s.clone())),

        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(Expr::array(vec![]));
            }

            let opcode = arr[0]
                .as_str()
                .ok_or_else(|| SExprError::ExpectedOpcode(format!("{:?}", arr[0])))?;

            let args = &arr[1..];
            parse_opcode_expr(opcode, args)
        }

        Value::Object(_) => Err(SExprError::InvalidArgument(
            "unexpected object in S-expression".into(),
        )),
    }
}

fn parse_opcode(opcode: &str, args: &[Value]) -> Result<Stmt, SExprError> {
    match opcode {
        // Statement opcodes
        "std.let" => {
            ensure_arity(opcode, args, 2)?;
            let name = args[0]
                .as_str()
                .ok_or_else(|| SExprError::InvalidArgument("std.let name must be string".into()))?;
            let init = value_to_expr(&args[1])?;
            Ok(Stmt::let_decl(name, Some(init)))
        }

        "std.seq" => {
            let stmts: Result<Vec<Stmt>, _> = args.iter().map(value_to_stmt).collect();
            Ok(Stmt::block(stmts?))
        }

        "std.if" => {
            if args.len() < 2 {
                return Err(SExprError::WrongArity {
                    opcode: opcode.into(),
                    expected: 2,
                    got: args.len(),
                });
            }
            let test = value_to_expr(&args[0])?;
            let consequent = value_to_stmt(&args[1])?;
            let alternate = if args.len() > 2 {
                Some(value_to_stmt(&args[2])?)
            } else {
                None
            };
            Ok(Stmt::if_stmt(test, consequent, alternate))
        }

        "std.while" => {
            ensure_arity(opcode, args, 2)?;
            let test = value_to_expr(&args[0])?;
            let body = value_to_stmt(&args[1])?;
            Ok(Stmt::while_loop(test, body))
        }

        "std.for" => {
            ensure_arity(opcode, args, 3)?;
            let var = args[0]
                .as_str()
                .ok_or_else(|| SExprError::InvalidArgument("std.for var must be string".into()))?;
            let iterable = value_to_expr(&args[1])?;
            let body = value_to_stmt(&args[2])?;
            Ok(Stmt::for_in(var, iterable, body))
        }

        "std.return" => {
            let expr = if args.is_empty() {
                None
            } else {
                Some(value_to_expr(&args[0])?)
            };
            Ok(Stmt::return_stmt(expr))
        }

        "std.break" => Ok(Stmt::break_stmt()),
        "std.continue" => Ok(Stmt::continue_stmt()),

        "std.fn" => {
            ensure_arity(opcode, args, 3)?;
            let name = args[0]
                .as_str()
                .ok_or_else(|| SExprError::InvalidArgument("std.fn name must be string".into()))?;
            let params = parse_params(&args[1])?;
            let body = value_to_stmt(&args[2])?;
            Ok(Stmt::function(Function::new(name, params, vec![body])))
        }

        // Expression opcodes - wrap in Stmt::Expr
        _ => {
            let expr = parse_opcode_expr(opcode, args)?;
            Ok(Stmt::expr(expr))
        }
    }
}

fn parse_opcode_expr(opcode: &str, args: &[Value]) -> Result<Expr, SExprError> {
    match opcode {
        // Variable access
        "std.var" => {
            ensure_arity(opcode, args, 1)?;
            let name = args[0]
                .as_str()
                .ok_or_else(|| SExprError::InvalidArgument("std.var name must be string".into()))?;
            Ok(Expr::ident(name))
        }

        // Variable assignment
        "std.set" => {
            ensure_arity(opcode, args, 2)?;
            let name = args[0]
                .as_str()
                .ok_or_else(|| SExprError::InvalidArgument("std.set name must be string".into()))?;
            let value = value_to_expr(&args[1])?;
            Ok(Expr::assign(Expr::ident(name), value))
        }

        // Arithmetic
        "math.add" => binary_op(args, BinaryOp::Add),
        "math.sub" => binary_op(args, BinaryOp::Sub),
        "math.mul" => binary_op(args, BinaryOp::Mul),
        "math.div" => binary_op(args, BinaryOp::Div),
        "math.mod" => binary_op(args, BinaryOp::Mod),
        "math.neg" => unary_op(args, UnaryOp::Neg),

        // Comparison
        "bool.eq" => binary_op(args, BinaryOp::Eq),
        "bool.neq" => binary_op(args, BinaryOp::Ne),
        "bool.lt" => binary_op(args, BinaryOp::Lt),
        "bool.lte" => binary_op(args, BinaryOp::Le),
        "bool.gt" => binary_op(args, BinaryOp::Gt),
        "bool.gte" => binary_op(args, BinaryOp::Ge),

        // Logical
        "bool.and" => binary_op(args, BinaryOp::And),
        "bool.or" => binary_op(args, BinaryOp::Or),
        "bool.not" => unary_op(args, UnaryOp::Not),

        // String
        "str.concat" => binary_op(args, BinaryOp::Concat),

        // Object/list operations
        "obj.get" => {
            ensure_arity(opcode, args, 2)?;
            let obj = value_to_expr(&args[0])?;
            let prop = value_to_expr(&args[1])?;
            Ok(Expr::Member {
                object: Box::new(obj),
                property: Box::new(prop),
                computed: true,
            })
        }

        "obj.set" => {
            ensure_arity(opcode, args, 3)?;
            let obj = value_to_expr(&args[0])?;
            let prop = value_to_expr(&args[1])?;
            let value = value_to_expr(&args[2])?;
            let target = Expr::Member {
                object: Box::new(obj),
                property: Box::new(prop),
                computed: true,
            };
            Ok(Expr::assign(target, value))
        }

        "obj.new" => {
            let pairs: Result<Vec<(String, Expr)>, _> = args
                .iter()
                .map(|pair| {
                    let arr = pair.as_array().ok_or_else(|| {
                        SExprError::InvalidArgument("obj.new pair must be array".into())
                    })?;
                    if arr.len() != 2 {
                        return Err(SExprError::InvalidArgument(
                            "obj.new pair must have 2 elements".into(),
                        ));
                    }
                    let key = arr[0].as_str().ok_or_else(|| {
                        SExprError::InvalidArgument("obj.new key must be string".into())
                    })?;
                    let val = value_to_expr(&arr[1])?;
                    Ok((key.to_string(), val))
                })
                .collect();
            Ok(Expr::object(pairs?))
        }

        "list.new" => {
            let items: Result<Vec<Expr>, _> = args.iter().map(value_to_expr).collect();
            Ok(Expr::array(items?))
        }

        "list.get" => {
            ensure_arity(opcode, args, 2)?;
            let arr = value_to_expr(&args[0])?;
            let idx = value_to_expr(&args[1])?;
            Ok(Expr::index(arr, idx))
        }

        // Lambda
        "std.lambda" => {
            ensure_arity(opcode, args, 2)?;
            let params = parse_params(&args[0])?;
            let body = value_to_stmt(&args[1])?;
            Ok(Expr::Function(Box::new(Function::anonymous(
                params,
                vec![body],
            ))))
        }

        // std.this
        "std.this" => Ok(Expr::ident("this")),

        // std.apply - dynamic function call
        "std.apply" => {
            if args.is_empty() {
                return Err(SExprError::WrongArity {
                    opcode: opcode.into(),
                    expected: 1,
                    got: 0,
                });
            }
            let callee = value_to_expr(&args[0])?;
            let call_args: Result<Vec<Expr>, _> = args[1..].iter().map(value_to_expr).collect();
            Ok(Expr::call(callee, call_args?))
        }

        // Generic function call (namespace.function)
        _ => {
            // Treat as a function call: opcode(args...)
            let parts: Vec<&str> = opcode.split('.').collect();
            let callee = if parts.len() == 1 {
                Expr::ident(parts[0])
            } else {
                // Build member expression chain: a.b.c
                let mut expr = Expr::ident(parts[0]);
                for part in &parts[1..] {
                    expr = Expr::member(expr, *part);
                }
                expr
            };
            let call_args: Result<Vec<Expr>, _> = args.iter().map(value_to_expr).collect();
            Ok(Expr::call(callee, call_args?))
        }
    }
}

fn binary_op(args: &[Value], op: BinaryOp) -> Result<Expr, SExprError> {
    if args.len() != 2 {
        return Err(SExprError::WrongArity {
            opcode: format!("{:?}", op),
            expected: 2,
            got: args.len(),
        });
    }
    let left = value_to_expr(&args[0])?;
    let right = value_to_expr(&args[1])?;
    Ok(Expr::binary(left, op, right))
}

fn unary_op(args: &[Value], op: UnaryOp) -> Result<Expr, SExprError> {
    if args.len() != 1 {
        return Err(SExprError::WrongArity {
            opcode: format!("{:?}", op),
            expected: 1,
            got: args.len(),
        });
    }
    Ok(Expr::unary(op, value_to_expr(&args[0])?))
}

fn parse_params(value: &Value) -> Result<Vec<String>, SExprError> {
    let arr = value
        .as_array()
        .ok_or_else(|| SExprError::InvalidArgument("params must be array".into()))?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| SExprError::InvalidArgument("param must be string".into()))
        })
        .collect()
}

fn ensure_arity(opcode: &str, args: &[Value], expected: usize) -> Result<(), SExprError> {
    if args.len() != expected {
        Err(SExprError::WrongArity {
            opcode: opcode.into(),
            expected,
            got: args.len(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_let() {
        let sexpr = json!(["std.let", "x", 42]);
        let program = from_sexpr(&sexpr).unwrap();
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
        let sexpr = json!(["math.add", 1, 2]);
        let program = from_sexpr(&sexpr).unwrap();
        match &program.body[0] {
            Stmt::Expr(Expr::Binary { op, .. }) => {
                assert_eq!(*op, BinaryOp::Add);
            }
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn test_function_call() {
        let sexpr = json!(["console.log", "hello"]);
        let program = from_sexpr(&sexpr).unwrap();
        match &program.body[0] {
            Stmt::Expr(Expr::Call { callee, args }) => {
                assert_eq!(args.len(), 1);
                // Callee should be console.log member expression
                match callee.as_ref() {
                    Expr::Member { .. } => {}
                    _ => panic!("expected Member expression"),
                }
            }
            _ => panic!("expected Call"),
        }
    }
}
