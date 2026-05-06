//! Statement types for the IR.

use super::{Expr, Span};
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
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Block: `{ stmts... }`.
    Block(Vec<Stmt>),

    /// If statement: `if (test) consequent else alternate`.
    If {
        test: Expr,
        consequent: Box<Stmt>,
        alternate: Option<Box<Stmt>>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// While loop: `while (test) body`.
    While {
        test: Expr,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// For loop: `for (init; test; update) body`.
    For {
        init: Option<Box<Stmt>>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// For-in/for-of loop: `for (variable in/of iterable) body`.
    ForIn {
        variable: String,
        iterable: Expr,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
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
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
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
            span: None,
        }
    }

    pub fn const_decl(name: impl Into<String>, init: Expr) -> Self {
        Stmt::Let {
            name: name.into(),
            init: Some(init),
            mutable: false,
            span: None,
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
            span: None,
        }
    }

    pub fn while_loop(test: Expr, body: Stmt) -> Self {
        Stmt::While {
            test,
            body: Box::new(body),
            span: None,
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
            span: None,
        }
    }

    pub fn for_in(variable: impl Into<String>, iterable: Expr, body: Stmt) -> Self {
        Stmt::ForIn {
            variable: variable.into(),
            iterable,
            body: Box::new(body),
            span: None,
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
            span: None,
        }
    }

    pub fn function(f: crate::Function) -> Self {
        Stmt::Function(f)
    }

    /// Attach a source location span to this statement.
    pub fn with_span(self, span: Span) -> Self {
        match self {
            Stmt::Let {
                name,
                init,
                mutable,
                ..
            } => Stmt::Let {
                name,
                init,
                mutable,
                span: Some(span),
            },
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => Stmt::If {
                test,
                consequent,
                alternate,
                span: Some(span),
            },
            Stmt::While { test, body, .. } => Stmt::While {
                test,
                body,
                span: Some(span),
            },
            Stmt::For {
                init,
                test,
                update,
                body,
                ..
            } => Stmt::For {
                init,
                test,
                update,
                body,
                span: Some(span),
            },
            Stmt::ForIn {
                variable,
                iterable,
                body,
                ..
            } => Stmt::ForIn {
                variable,
                iterable,
                body,
                span: Some(span),
            },
            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
                ..
            } => Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
                span: Some(span),
            },
            other => other,
        }
    }
}
