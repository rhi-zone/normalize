//! Snapshot tests for readers and writers.
//!
//! These tests verify that parsing and emission produce expected output.
//! Run `cargo insta review` to update snapshots after intentional changes.

use normalize_surface_syntax::{Expr, Program, Stmt};

// ============================================================================
// Reader Snapshots - verify parsed IR is correct
// ============================================================================

mod typescript_reader {
    use super::*;
    use normalize_surface_syntax::input::read_typescript;

    fn parse(code: &str) -> Program {
        read_typescript(code).expect("parse failed")
    }

    #[test]
    fn variable_declaration() {
        insta::assert_json_snapshot!(parse("const x = 42;"));
    }

    #[test]
    fn let_declaration() {
        insta::assert_json_snapshot!(parse("let y = \"hello\";"));
    }

    #[test]
    fn binary_expression() {
        insta::assert_json_snapshot!(parse("const result = 1 + 2 * 3;"));
    }

    #[test]
    fn comparison_operators() {
        insta::assert_json_snapshot!(parse("const check = x > 0 && y <= 10;"));
    }

    #[test]
    fn function_call() {
        insta::assert_json_snapshot!(parse("console.log(\"hello\", 42);"));
    }

    #[test]
    fn nested_calls() {
        insta::assert_json_snapshot!(parse("Math.max(Math.min(x, 10), 0);"));
    }

    #[test]
    fn if_statement() {
        insta::assert_json_snapshot!(parse("if (x > 0) { console.log(x); }"));
    }

    #[test]
    fn if_else_statement() {
        insta::assert_json_snapshot!(parse(
            "if (x > 0) { console.log(\"positive\"); } else { console.log(\"non-positive\"); }"
        ));
    }

    #[test]
    fn while_loop() {
        insta::assert_json_snapshot!(parse("while (i < 10) { i = i + 1; }"));
    }

    #[test]
    fn for_loop() {
        insta::assert_json_snapshot!(parse(
            "for (let i = 0; i < 10; i = i + 1) { console.log(i); }"
        ));
    }

    #[test]
    fn arrow_function() {
        insta::assert_json_snapshot!(parse("const add = (a, b) => a + b;"));
    }

    #[test]
    fn array_literal() {
        insta::assert_json_snapshot!(parse("const arr = [1, 2, 3];"));
    }

    #[test]
    fn object_literal() {
        insta::assert_json_snapshot!(parse("const obj = { x: 1, y: 2 };"));
    }

    #[test]
    fn function_declaration() {
        insta::assert_json_snapshot!(parse("function greet(name) { return \"Hello, \" + name; }"));
    }

    #[test]
    fn try_catch() {
        insta::assert_json_snapshot!(parse(
            "try { doSomething(); } catch (e) { console.log(e); }"
        ));
    }

    #[test]
    fn try_catch_finally() {
        insta::assert_json_snapshot!(parse(
            "try { doSomething(); } catch (e) { console.log(e); } finally { cleanup(); }"
        ));
    }

    #[test]
    fn try_finally() {
        insta::assert_json_snapshot!(parse("try { doSomething(); } finally { cleanup(); }"));
    }
}

mod lua_reader {
    use super::*;
    use normalize_surface_syntax::input::read_lua;

    fn parse(code: &str) -> Program {
        read_lua(code).expect("parse failed")
    }

    #[test]
    fn local_variable() {
        insta::assert_json_snapshot!(parse("local x = 42"));
    }

    #[test]
    fn string_variable() {
        insta::assert_json_snapshot!(parse("local y = \"hello\""));
    }

    #[test]
    fn binary_expression() {
        insta::assert_json_snapshot!(parse("local result = 1 + 2 * 3"));
    }

    #[test]
    fn logical_operators() {
        insta::assert_json_snapshot!(parse("local check = x > 0 and y <= 10"));
    }

    #[test]
    fn function_call() {
        insta::assert_json_snapshot!(parse("print(\"hello\", 42)"));
    }

    #[test]
    fn method_call() {
        insta::assert_json_snapshot!(parse("console.log(\"hello\")"));
    }

    #[test]
    fn if_statement() {
        insta::assert_json_snapshot!(parse("if x > 0 then print(x) end"));
    }

    #[test]
    fn if_else_statement() {
        insta::assert_json_snapshot!(parse(
            "if x > 0 then print(\"positive\") else print(\"non-positive\") end"
        ));
    }

    #[test]
    fn while_loop() {
        insta::assert_json_snapshot!(parse("while i < 10 do i = i + 1 end"));
    }

    #[test]
    fn numeric_for() {
        insta::assert_json_snapshot!(parse("for i = 1, 10 do print(i) end"));
    }

    #[test]
    fn table_array() {
        insta::assert_json_snapshot!(parse("local arr = {1, 2, 3}"));
    }

    #[test]
    fn table_record() {
        insta::assert_json_snapshot!(parse("local obj = {x = 1, y = 2}"));
    }

    #[test]
    fn function_declaration() {
        insta::assert_json_snapshot!(parse("function greet(name) return \"Hello, \" .. name end"));
    }

    #[test]
    fn anonymous_function() {
        insta::assert_json_snapshot!(parse("local add = function(a, b) return a + b end"));
    }
}

// ============================================================================
// Writer Snapshots - verify emitted code is correct
// ============================================================================

mod lua_writer {
    use super::*;
    use normalize_surface_syntax::output::lua::LuaWriter;

    fn emit(program: &Program) -> String {
        LuaWriter::emit(program)
    }

    fn ir_const(name: &str, value: Expr) -> Program {
        Program {
            body: vec![Stmt::const_decl(name, value)],
        }
    }

    #[test]
    fn simple_number() {
        insta::assert_snapshot!(emit(&ir_const("x", Expr::number(42))));
    }

    #[test]
    fn simple_string() {
        insta::assert_snapshot!(emit(&ir_const("msg", Expr::string("hello"))));
    }

    #[test]
    fn binary_add() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&ir_const(
            "sum",
            Expr::binary(Expr::number(1), BinaryOp::Add, Expr::number(2))
        )));
    }

    #[test]
    fn function_call() {
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::expr(Expr::call(
                Expr::ident("print"),
                vec![Expr::string("hello")]
            ))]
        }));
    }

    #[test]
    fn if_statement() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::if_stmt(
                Expr::binary(Expr::ident("x"), BinaryOp::Gt, Expr::number(0)),
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("print"),
                    vec![Expr::ident("x")]
                ))]),
                None
            )]
        }));
    }

    #[test]
    fn while_loop() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::while_loop(
                Expr::binary(Expr::ident("i"), BinaryOp::Lt, Expr::number(10)),
                Stmt::block(vec![Stmt::expr(Expr::assign(
                    Expr::ident("i"),
                    Expr::binary(Expr::ident("i"), BinaryOp::Add, Expr::number(1))
                ))])
            )]
        }));
    }
}

mod lua_try_catch {
    use super::*;
    use normalize_surface_syntax::output::lua::LuaWriter;

    fn emit(program: &Program) -> String {
        LuaWriter::emit(program)
    }

    #[test]
    fn try_catch_finally() {
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::try_catch(
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("doSomething"),
                    vec![]
                ))]),
                Some("e".into()),
                Some(Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::member(Expr::ident("console"), "log"),
                    vec![Expr::ident("e")]
                ))])),
                Some(Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("cleanup"),
                    vec![]
                ))]))
            )]
        }));
    }

    #[test]
    fn try_finally() {
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::try_catch(
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("doSomething"),
                    vec![]
                ))]),
                None,
                None,
                Some(Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("cleanup"),
                    vec![]
                ))]))
            )]
        }));
    }
}

mod typescript_writer {
    use super::*;
    use normalize_surface_syntax::output::typescript::TypeScriptWriter;

    fn emit(program: &Program) -> String {
        TypeScriptWriter::emit(program)
    }

    fn ir_const(name: &str, value: Expr) -> Program {
        Program {
            body: vec![Stmt::const_decl(name, value)],
        }
    }

    fn ir_let(name: &str, value: Expr) -> Program {
        Program {
            body: vec![Stmt::let_decl(name, Some(value))],
        }
    }

    #[test]
    fn const_number() {
        insta::assert_snapshot!(emit(&ir_const("x", Expr::number(42))));
    }

    #[test]
    fn let_string() {
        insta::assert_snapshot!(emit(&ir_let("msg", Expr::string("hello"))));
    }

    #[test]
    fn binary_add() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&ir_const(
            "sum",
            Expr::binary(Expr::number(1), BinaryOp::Add, Expr::number(2))
        )));
    }

    #[test]
    fn function_call() {
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::expr(Expr::call(
                Expr::member(Expr::ident("console"), "log"),
                vec![Expr::string("hello")]
            ))]
        }));
    }

    #[test]
    fn if_statement() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::if_stmt(
                Expr::binary(Expr::ident("x"), BinaryOp::Gt, Expr::number(0)),
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::member(Expr::ident("console"), "log"),
                    vec![Expr::ident("x")]
                ))]),
                None
            )]
        }));
    }

    #[test]
    fn for_loop() {
        use normalize_surface_syntax::BinaryOp;
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::for_loop(
                Some(Stmt::let_decl("i", Some(Expr::number(0)))),
                Some(Expr::binary(
                    Expr::ident("i"),
                    BinaryOp::Lt,
                    Expr::number(10)
                )),
                Some(Expr::assign(
                    Expr::ident("i"),
                    Expr::binary(Expr::ident("i"), BinaryOp::Add, Expr::number(1))
                )),
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::member(Expr::ident("console"), "log"),
                    vec![Expr::ident("i")]
                ))])
            )]
        }));
    }

    #[test]
    fn object_literal() {
        insta::assert_snapshot!(emit(&ir_const(
            "obj",
            Expr::object(vec![
                ("x".into(), Expr::number(1)),
                ("y".into(), Expr::number(2))
            ])
        )));
    }

    #[test]
    fn array_literal() {
        insta::assert_snapshot!(emit(&ir_const(
            "arr",
            Expr::array(vec![Expr::number(1), Expr::number(2), Expr::number(3)])
        )));
    }

    #[test]
    fn try_catch_finally() {
        insta::assert_snapshot!(emit(&Program {
            body: vec![Stmt::try_catch(
                Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::member(Expr::ident("console"), "log"),
                    vec![Expr::string("trying")]
                ))]),
                Some("e".into()),
                Some(Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::member(Expr::ident("console"), "error"),
                    vec![Expr::ident("e")]
                ))])),
                Some(Stmt::block(vec![Stmt::expr(Expr::call(
                    Expr::ident("cleanup"),
                    vec![]
                ))]))
            )]
        }));
    }
}
