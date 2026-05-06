//! Pattern types for the IR.
//!
//! Patterns appear in destructuring bindings (`let { a, b } = obj`) and
//! in function parameters (`function f({ name }) {}`).

use super::Expr;
use serde::{Deserialize, Serialize};

/// A binding pattern.
///
/// Patterns appear on the left-hand side of destructuring declarations and
/// in destructuring function parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pat {
    /// Simple identifier binding: `x`.
    Ident(String),

    /// Object destructuring pattern: `{ x, y: z, ...rest }`.
    Object(Vec<PatField>),

    /// Array/tuple destructuring pattern: `[x, y, ...rest]`.
    ///
    /// Elements are `None` for holes (e.g. `[, y]` skips the first element).
    /// `rest` is the name of the rest element (e.g. `"rest"` for `[...rest]`).
    Array(Vec<Option<Pat>>, Option<String>),

    /// Rest pattern: `...rest` (used inside object/array patterns).
    Rest(Box<Pat>),
}

/// A field in an object destructuring pattern.
///
/// Represents one entry in `{ key: pat = default }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatField {
    /// The source property key (e.g. `"b"` in `{ b: c }`).
    pub key: String,
    /// The binding target pattern (e.g. `Pat::Ident("c")` in `{ b: c }`).
    pub pat: Pat,
    /// Optional default value (e.g. `"foo"` in `{ a = "foo" }`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Expr>,
}

impl Pat {
    /// Create a simple identifier pattern.
    pub fn ident(name: impl Into<String>) -> Self {
        Pat::Ident(name.into())
    }

    /// Create an object pattern from a list of fields.
    pub fn object(fields: Vec<PatField>) -> Self {
        Pat::Object(fields)
    }

    /// Create an array pattern from elements and an optional rest name.
    pub fn array(elements: Vec<Option<Pat>>, rest: Option<String>) -> Self {
        Pat::Array(elements, rest)
    }
}

impl PatField {
    /// Create a shorthand field: `{ key }` binds `key` to `Pat::Ident(key)`.
    pub fn shorthand(key: impl Into<String>) -> Self {
        let k = key.into();
        Self {
            pat: Pat::Ident(k.clone()),
            key: k,
            default: None,
        }
    }

    /// Create a renamed field: `{ key: name }`.
    pub fn renamed(key: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            pat: Pat::Ident(name.into()),
            default: None,
        }
    }

    /// Create a field with a nested pattern: `{ key: pat }`.
    pub fn nested(key: impl Into<String>, pat: Pat) -> Self {
        Self {
            key: key.into(),
            pat,
            default: None,
        }
    }

    /// Attach a default value to this field.
    pub fn with_default(mut self, default: Expr) -> Self {
        self.default = Some(default);
        self
    }
}
