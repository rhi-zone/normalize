//! Expression types for the IR.

use serde::{Deserialize, Serialize};

/// An expression that produces a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Literal value.
    Literal(Literal),

    /// Variable reference.
    Ident(String),

    /// Binary operation: `left op right`.
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },

    /// Unary operation: `op expr`.
    Unary { op: UnaryOp, expr: Box<Expr> },

    /// Function call: `callee(args...)`.
    Call { callee: Box<Expr>, args: Vec<Expr> },

    /// Member access: `object.property` or `object[property]`.
    Member {
        object: Box<Expr>,
        property: Box<Expr>,
        /// True for `obj[expr]`, false for `obj.ident`.
        computed: bool,
    },

    /// Array literal: `[a, b, c]`.
    Array(Vec<Expr>),

    /// Object literal: `{ key: value, ... }`.
    Object(Vec<(String, Expr)>),

    /// Anonymous function: `function(params) { body }`.
    Function(Box<crate::Function>),

    /// Ternary/conditional: `cond ? then : else`.
    Conditional {
        test: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
    },

    /// Assignment: `target = value`.
    Assign { target: Box<Expr>, value: Box<Expr> },
}

/// Literal values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,

    // String
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

// Builder methods for expressions
impl Expr {
    pub fn null() -> Self {
        Expr::Literal(Literal::Null)
    }

    pub fn bool(v: bool) -> Self {
        Expr::Literal(Literal::Bool(v))
    }

    pub fn number(v: impl Into<f64>) -> Self {
        Expr::Literal(Literal::Number(v.into()))
    }

    pub fn string(v: impl Into<String>) -> Self {
        Expr::Literal(Literal::String(v.into()))
    }

    pub fn ident(name: impl Into<String>) -> Self {
        Expr::Ident(name.into())
    }

    pub fn binary(left: Expr, op: BinaryOp, right: Expr) -> Self {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    pub fn unary(op: UnaryOp, expr: Expr) -> Self {
        Expr::Unary {
            op,
            expr: Box::new(expr),
        }
    }

    pub fn call(callee: Expr, args: Vec<Expr>) -> Self {
        Expr::Call {
            callee: Box::new(callee),
            args,
        }
    }

    pub fn member(object: Expr, property: impl Into<String>) -> Self {
        Expr::Member {
            object: Box::new(object),
            property: Box::new(Expr::string(property)),
            computed: false,
        }
    }

    pub fn index(object: Expr, index: Expr) -> Self {
        Expr::Member {
            object: Box::new(object),
            property: Box::new(index),
            computed: true,
        }
    }

    pub fn array(items: Vec<Expr>) -> Self {
        Expr::Array(items)
    }

    pub fn object(pairs: Vec<(String, Expr)>) -> Self {
        Expr::Object(pairs)
    }

    pub fn conditional(test: Expr, consequent: Expr, alternate: Expr) -> Self {
        Expr::Conditional {
            test: Box::new(test),
            consequent: Box::new(consequent),
            alternate: Box::new(alternate),
        }
    }

    pub fn assign(target: Expr, value: Expr) -> Self {
        Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
        }
    }
}
