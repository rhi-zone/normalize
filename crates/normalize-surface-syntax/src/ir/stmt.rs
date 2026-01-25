//! Statement types for the IR.

use crate::Expr;
use serde::{Deserialize, Serialize};

/// A statement (doesn't produce a value directly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// Expression statement: `expr;`.
    Expr(Expr),

    /// Variable declaration: `let name = init` or `const name = init`.
    Let {
        name: String,
        init: Option<Expr>,
        mutable: bool,
    },

    /// Block: `{ stmts... }`.
    Block(Vec<Stmt>),

    /// If statement: `if (test) consequent else alternate`.
    If {
        test: Expr,
        consequent: Box<Stmt>,
        alternate: Option<Box<Stmt>>,
    },

    /// While loop: `while (test) body`.
    While { test: Expr, body: Box<Stmt> },

    /// For loop: `for (init; test; update) body`.
    For {
        init: Option<Box<Stmt>>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },

    /// For-in/for-of loop: `for (variable in/of iterable) body`.
    ForIn {
        variable: String,
        iterable: Expr,
        body: Box<Stmt>,
    },

    /// Return statement: `return expr`.
    Return(Option<Expr>),

    /// Break statement.
    Break,

    /// Continue statement.
    Continue,

    /// Try/catch/finally statement.
    TryCatch {
        body: Box<Stmt>,
        catch_param: Option<String>,
        catch_body: Option<Box<Stmt>>,
        finally_body: Option<Box<Stmt>>,
    },

    /// Function declaration.
    Function(crate::Function),
}

// Builder methods for statements
impl Stmt {
    pub fn expr(e: Expr) -> Self {
        Stmt::Expr(e)
    }

    pub fn let_decl(name: impl Into<String>, init: Option<Expr>) -> Self {
        Stmt::Let {
            name: name.into(),
            init,
            mutable: true,
        }
    }

    pub fn const_decl(name: impl Into<String>, init: Expr) -> Self {
        Stmt::Let {
            name: name.into(),
            init: Some(init),
            mutable: false,
        }
    }

    pub fn block(stmts: Vec<Stmt>) -> Self {
        Stmt::Block(stmts)
    }

    pub fn if_stmt(test: Expr, consequent: Stmt, alternate: Option<Stmt>) -> Self {
        Stmt::If {
            test,
            consequent: Box::new(consequent),
            alternate: alternate.map(Box::new),
        }
    }

    pub fn while_loop(test: Expr, body: Stmt) -> Self {
        Stmt::While {
            test,
            body: Box::new(body),
        }
    }

    pub fn for_loop(
        init: Option<Stmt>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Stmt,
    ) -> Self {
        Stmt::For {
            init: init.map(Box::new),
            test,
            update,
            body: Box::new(body),
        }
    }

    pub fn for_in(variable: impl Into<String>, iterable: Expr, body: Stmt) -> Self {
        Stmt::ForIn {
            variable: variable.into(),
            iterable,
            body: Box::new(body),
        }
    }

    pub fn return_stmt(expr: Option<Expr>) -> Self {
        Stmt::Return(expr)
    }

    pub fn break_stmt() -> Self {
        Stmt::Break
    }

    pub fn continue_stmt() -> Self {
        Stmt::Continue
    }

    pub fn try_catch(
        body: Stmt,
        catch_param: Option<String>,
        catch_body: Option<Stmt>,
        finally_body: Option<Stmt>,
    ) -> Self {
        Stmt::TryCatch {
            body: Box::new(body),
            catch_param,
            catch_body: catch_body.map(Box::new),
            finally_body: finally_body.map(Box::new),
        }
    }

    pub fn function(f: crate::Function) -> Self {
        Stmt::Function(f)
    }
}
