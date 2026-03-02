//! Type reference types for code facts.

use serde::{Deserialize, Serialize};

/// The kind of type reference relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeRefKind {
    /// struct/class field uses this type
    FieldType,
    /// function parameter type
    ParamType,
    /// function return type
    ReturnType,
    /// class extends / struct embeds
    Extends,
    /// implements trait/interface
    Implements,
    /// generic constraint (T: Foo)
    GenericBound,
    /// type alias target
    TypeAlias,
}

impl TypeRefKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TypeRefKind::FieldType => "field_type",
            TypeRefKind::ParamType => "param_type",
            TypeRefKind::ReturnType => "return_type",
            TypeRefKind::Extends => "extends",
            TypeRefKind::Implements => "implements",
            TypeRefKind::GenericBound => "generic_bound",
            TypeRefKind::TypeAlias => "type_alias",
        }
    }
}

/// A type-to-type reference extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRef {
    /// The type/function containing the reference
    pub source_symbol: String,
    /// The referenced type name
    pub target_type: String,
    /// What kind of reference this is
    pub kind: TypeRefKind,
    /// Line number of the reference
    pub line: usize,
}
