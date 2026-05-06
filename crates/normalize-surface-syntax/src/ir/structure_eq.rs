//! Structural equality for IR types.
//!
//! `structure_eq` compares IR trees ignoring "surface hints" - fields that
//! capture language-specific details but don't affect program semantics.
//!
//! # Hint Fields (normalized during comparison)
//!
//! - `Stmt::Let { mutable }` - Lua doesn't distinguish const/let
//! - `Expr::Member { computed }` - normalized to false when property is string literal
//!
//! # Core Fields (must match exactly)
//!
//! - All names, values, operators
//! - Control flow structure
//! - Expression trees

use crate::{Expr, Function, Method, Program, Stmt, TemplatePart};

/// Trait for structural equality comparison.
///
/// Unlike `PartialEq`, this ignores surface hint fields that may differ
/// between languages but don't affect program semantics.
pub trait StructureEq {
    /// Compare two values for structural equality.
    fn structure_eq(&self, other: &Self) -> bool;
}

impl StructureEq for Program {
    fn structure_eq(&self, other: &Self) -> bool {
        self.body.len() == other.body.len()
            && self
                .body
                .iter()
                .zip(&other.body)
                .all(|(a, b)| a.structure_eq(b))
    }
}

impl StructureEq for Stmt {
    fn structure_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Stmt::Expr(a), Stmt::Expr(b)) => a.structure_eq(b),

            // Ignore `mutable`, `type_annotation`, and `span` - they are surface hints
            (
                Stmt::Let {
                    name: n1,
                    init: i1,
                    mutable: _,
                    type_annotation: _,
                    span: _,
                },
                Stmt::Let {
                    name: n2,
                    init: i2,
                    mutable: _,
                    type_annotation: _,
                    span: _,
                },
            ) => n1 == n2 && option_structure_eq(i1.as_ref(), i2.as_ref()),

            (Stmt::Block(a), Stmt::Block(b)) => vec_structure_eq(a, b),

            (
                Stmt::If {
                    test: t1,
                    consequent: c1,
                    alternate: a1,
                    span: _,
                },
                Stmt::If {
                    test: t2,
                    consequent: c2,
                    alternate: a2,
                    span: _,
                },
            ) => {
                t1.structure_eq(t2)
                    && c1.structure_eq(c2.as_ref())
                    && match (a1, a2) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a.structure_eq(b.as_ref()),
                        _ => false,
                    }
            }

            (
                Stmt::While {
                    test: t1,
                    body: b1,
                    span: _,
                },
                Stmt::While {
                    test: t2,
                    body: b2,
                    span: _,
                },
            ) => t1.structure_eq(t2) && b1.structure_eq(b2.as_ref()),

            (
                Stmt::For {
                    init: i1,
                    test: t1,
                    update: u1,
                    body: b1,
                    span: _,
                },
                Stmt::For {
                    init: i2,
                    test: t2,
                    update: u2,
                    body: b2,
                    span: _,
                },
            ) => {
                (match (i1, i2) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.structure_eq(b.as_ref()),
                    _ => false,
                }) && option_structure_eq(t1.as_ref(), t2.as_ref())
                    && option_structure_eq(u1.as_ref(), u2.as_ref())
                    && b1.structure_eq(b2.as_ref())
            }

            (
                Stmt::ForIn {
                    variable: v1,
                    iterable: i1,
                    body: b1,
                    span: _,
                },
                Stmt::ForIn {
                    variable: v2,
                    iterable: i2,
                    body: b2,
                    span: _,
                },
            ) => v1 == v2 && i1.structure_eq(i2) && b1.structure_eq(b2.as_ref()),

            (Stmt::Return(a), Stmt::Return(b)) => option_structure_eq(a.as_ref(), b.as_ref()),

            (Stmt::Break, Stmt::Break) => true,
            (Stmt::Continue, Stmt::Continue) => true,

            (
                Stmt::TryCatch {
                    body: b1,
                    catch_param: cp1,
                    catch_body: cb1,
                    finally_body: fb1,
                    span: _,
                },
                Stmt::TryCatch {
                    body: b2,
                    catch_param: cp2,
                    catch_body: cb2,
                    finally_body: fb2,
                    span: _,
                },
            ) => {
                b1.structure_eq(b2.as_ref())
                    && cp1 == cp2
                    && match (cb1, cb2) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a.structure_eq(b.as_ref()),
                        _ => false,
                    }
                    && match (fb1, fb2) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a.structure_eq(b.as_ref()),
                        _ => false,
                    }
            }

            (Stmt::Function(a), Stmt::Function(b)) => a.structure_eq(b),

            // Comments: compare text and block flag; ignore span
            (
                Stmt::Comment {
                    text: t1,
                    block: b1,
                    span: _,
                },
                Stmt::Comment {
                    text: t2,
                    block: b2,
                    span: _,
                },
            ) => t1 == t2 && b1 == b2,

            // Import: compare source and names (ignore span)
            (
                Stmt::Import {
                    source: s1,
                    names: n1,
                    span: _,
                },
                Stmt::Import {
                    source: s2,
                    names: n2,
                    span: _,
                },
            ) => s1 == s2 && n1 == n2,

            // Export: compare names and source (ignore span)
            (
                Stmt::Export {
                    names: n1,
                    source: s1,
                    span: _,
                },
                Stmt::Export {
                    names: n2,
                    source: s2,
                    span: _,
                },
            ) => n1 == n2 && s1 == s2,

            // Class: compare name, extends, and methods (ignore span)
            (
                Stmt::Class {
                    name: n1,
                    extends: e1,
                    methods: m1,
                    span: _,
                },
                Stmt::Class {
                    name: n2,
                    extends: e2,
                    methods: m2,
                    span: _,
                },
            ) => {
                n1 == n2
                    && e1 == e2
                    && m1.len() == m2.len()
                    && m1.iter().zip(m2).all(|(a, b)| a.structure_eq(b))
            }

            _ => false,
        }
    }
}

impl StructureEq for Expr {
    fn structure_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Expr::Literal(a), Expr::Literal(b)) => a == b,
            (Expr::Ident(a), Expr::Ident(b)) => a == b,

            (
                Expr::Binary {
                    left: l1,
                    op: o1,
                    right: r1,
                    span: _,
                },
                Expr::Binary {
                    left: l2,
                    op: o2,
                    right: r2,
                    span: _,
                },
            ) => o1 == o2 && l1.structure_eq(l2) && r1.structure_eq(r2),

            (
                Expr::Unary {
                    op: o1,
                    expr: e1,
                    span: _,
                },
                Expr::Unary {
                    op: o2,
                    expr: e2,
                    span: _,
                },
            ) => o1 == o2 && e1.structure_eq(e2),

            (
                Expr::Call {
                    callee: c1,
                    args: a1,
                    span: _,
                },
                Expr::Call {
                    callee: c2,
                    args: a2,
                    span: _,
                },
            ) => c1.structure_eq(c2) && vec_structure_eq(a1, a2),

            // Normalize `computed` when property is a string literal; ignore `span`
            (
                Expr::Member {
                    object: o1,
                    property: p1,
                    computed: _,
                    span: _,
                },
                Expr::Member {
                    object: o2,
                    property: p2,
                    computed: _,
                    span: _,
                },
            ) => o1.structure_eq(o2) && p1.structure_eq(p2),

            (Expr::Array(a), Expr::Array(b)) => vec_structure_eq(a, b),

            (Expr::Object(a), Expr::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((k1, v1), (k2, v2))| k1 == k2 && v1.structure_eq(v2))
            }

            (Expr::Function(a), Expr::Function(b)) => a.structure_eq(b),

            (
                Expr::Conditional {
                    test: t1,
                    consequent: c1,
                    alternate: a1,
                    span: _,
                },
                Expr::Conditional {
                    test: t2,
                    consequent: c2,
                    alternate: a2,
                    span: _,
                },
            ) => t1.structure_eq(t2) && c1.structure_eq(c2) && a1.structure_eq(a2),

            (
                Expr::Assign {
                    target: t1,
                    value: v1,
                    span: _,
                },
                Expr::Assign {
                    target: t2,
                    value: v2,
                    span: _,
                },
            ) => t1.structure_eq(t2) && v1.structure_eq(v2),

            (Expr::TemplateLiteral(a), Expr::TemplateLiteral(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.structure_eq(y))
            }

            _ => false,
        }
    }
}

impl StructureEq for TemplatePart {
    fn structure_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TemplatePart::Text(a), TemplatePart::Text(b)) => a == b,
            (TemplatePart::Expr(a), TemplatePart::Expr(b)) => a.structure_eq(b),
            _ => false,
        }
    }
}

impl StructureEq for Function {
    fn structure_eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.params.len() == other.params.len()
            && self
                .params
                .iter()
                .zip(&other.params)
                .all(|(a, b)| a.name == b.name)
            && vec_structure_eq(&self.body, &other.body)
    }
}

impl StructureEq for Method {
    fn structure_eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.is_static == other.is_static
            && self.params.len() == other.params.len()
            && self
                .params
                .iter()
                .zip(&other.params)
                .all(|(a, b)| a.name == b.name)
            && vec_structure_eq(&self.body, &other.body)
    }
}

// Helper functions

fn vec_structure_eq<T: StructureEq>(a: &[T], b: &[T]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.structure_eq(y))
}

fn option_structure_eq<T: StructureEq>(a: Option<&T>, b: Option<&T>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.structure_eq(y),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutable_is_ignored() {
        let const_decl = Stmt::Let {
            name: "x".into(),
            init: Some(Expr::number(42)),
            mutable: false,
            type_annotation: None,
            span: None,
        };
        let let_decl = Stmt::Let {
            name: "x".into(),
            init: Some(Expr::number(42)),
            mutable: true,
            type_annotation: None,
            span: None,
        };

        assert!(const_decl.structure_eq(&let_decl));
        assert_ne!(const_decl, let_decl); // Regular equality still differs
    }

    #[test]
    fn test_computed_is_ignored() {
        let dot_access = Expr::Member {
            object: Box::new(Expr::ident("obj")),
            property: Box::new(Expr::string("foo")),
            computed: false,
            span: None,
        };
        let bracket_access = Expr::Member {
            object: Box::new(Expr::ident("obj")),
            property: Box::new(Expr::string("foo")),
            computed: true,
            span: None,
        };

        assert!(dot_access.structure_eq(&bracket_access));
        assert_ne!(dot_access, bracket_access); // Regular equality still differs
    }

    #[test]
    fn test_different_names_not_equal() {
        let x = Stmt::Let {
            name: "x".into(),
            init: Some(Expr::number(1)),
            mutable: false,
            type_annotation: None,
            span: None,
        };
        let y = Stmt::Let {
            name: "y".into(),
            init: Some(Expr::number(1)),
            mutable: false,
            type_annotation: None,
            span: None,
        };

        assert!(!x.structure_eq(&y));
    }

    #[test]
    fn test_program_equality() {
        let p1 = Program {
            body: vec![Stmt::const_decl("x", Expr::number(1))],
        };
        let p2 = Program {
            body: vec![Stmt::let_decl("x", Some(Expr::number(1)))],
        };

        assert!(p1.structure_eq(&p2));
    }
}
