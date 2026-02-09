//! Symbol types for code facts.

use serde::{Deserialize, Serialize};

/// Symbol kind classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Module,
    Type,
    Constant,
    Variable,
    Heading,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Module => "module",
            SymbolKind::Type => "type",
            SymbolKind::Constant => "constant",
            SymbolKind::Variable => "variable",
            SymbolKind::Heading => "heading",
        }
    }
}

/// Symbol visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
        }
    }
}

/// How a language determines symbol visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityMechanism {
    /// Explicit export keyword (JS/TS: `export function foo()`)
    ExplicitExport,
    /// Access modifier keywords (Java, Scala, C#: `public`, `private`, `protected`)
    AccessModifier,
    /// Naming convention (Go: uppercase = public, Python: underscore = private)
    NamingConvention,
    /// Header-based (C/C++: symbols in headers are public, source files are private)
    HeaderBased,
    /// Everything is public by default (Ruby modules, Lua)
    AllPublic,
    /// Not applicable (data formats like JSON, YAML, TOML)
    NotApplicable,
}

/// A code symbol extracted from source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub docstring: Option<String>,
    pub attributes: Vec<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub visibility: Visibility,
    pub children: Vec<Symbol>,
    /// True if this symbol implements an interface/trait (e.g., method in `impl Trait for Type`)
    pub is_interface_impl: bool,
    /// Parent interfaces/classes this symbol extends or implements (for semantic matching)
    pub implements: Vec<String>,
}

/// A flattened symbol for indexing (parent reference instead of nested children)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub parent: Option<String>,
    pub visibility: Visibility,
    pub attributes: Vec<String>,
}
