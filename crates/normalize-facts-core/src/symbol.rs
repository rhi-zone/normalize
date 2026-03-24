//! Symbol types for code facts.

use serde::{Deserialize, Serialize};

/// Symbol kind classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    /// A standalone function or procedure.
    Function,
    /// A method belonging to a class, struct, or impl block.
    Method,
    /// A class definition (OOP languages).
    Class,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition (Rust) or abstract interface.
    Trait,
    /// An interface definition (Java, Go, TypeScript).
    Interface,
    /// A module, namespace, or package declaration.
    Module,
    /// A type alias or type definition.
    Type,
    /// A constant or compile-time value.
    Constant,
    /// A variable declaration.
    Variable,
    /// A Markdown heading (used to represent document sections as symbols).
    Heading,
}

impl SymbolKind {
    /// Returns the lowercase string representation of this symbol kind.
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
    /// Exported / accessible everywhere (default).
    #[default]
    Public,
    /// Accessible only within the defining scope or file.
    Private,
    /// Accessible to the defining type and its subclasses.
    Protected,
    /// Accessible within the same package or crate but not externally.
    Internal,
}

impl Visibility {
    /// Returns the lowercase string representation of this visibility level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
        }
    }
}

/// A code symbol extracted from source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// The symbol's unqualified name.
    pub name: String,
    /// Classification of the symbol (function, class, heading, etc.).
    pub kind: SymbolKind,
    /// Full signature string (e.g., `fn foo(x: i32) -> bool`). Empty if not applicable.
    pub signature: String,
    /// Documentation comment or docstring attached to this symbol, if present.
    pub docstring: Option<String>,
    /// Language-specific decorators, annotations, or attributes (e.g., `#[derive(...)]` in Rust,
    /// `@decorator` in Python). Each entry is the raw text of one attribute.
    pub attributes: Vec<String>,
    /// 1-based line number where the symbol starts.
    pub start_line: usize,
    /// 1-based line number where the symbol ends (inclusive).
    pub end_line: usize,
    /// Visibility of the symbol.
    pub visibility: Visibility,
    /// Nested symbols (e.g., methods inside a class). Empty for leaf symbols.
    pub children: Vec<Symbol>,
    /// True if this symbol implements an interface/trait (e.g., method in `impl Trait for Type`)
    pub is_interface_impl: bool,
    /// Parent interfaces/classes this symbol extends or implements (for semantic matching)
    pub implements: Vec<String>,
}

/// A flattened symbol for indexing (parent reference instead of nested children)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatSymbol {
    /// The symbol's unqualified name.
    pub name: String,
    /// Classification of the symbol.
    pub kind: SymbolKind,
    /// 1-based line number where the symbol starts.
    pub start_line: usize,
    /// 1-based line number where the symbol ends (inclusive).
    pub end_line: usize,
    /// Name of the enclosing symbol (e.g., the class for a method), if any.
    pub parent: Option<String>,
    /// Visibility of the symbol.
    pub visibility: Visibility,
    /// Language-specific decorators or annotations (raw text, one per entry).
    pub attributes: Vec<String>,
    /// True if this symbol implements an interface/trait
    pub is_interface_impl: bool,
    /// Parent interfaces/classes this symbol extends or implements
    pub implements: Vec<String>,
}
