//! Shared types and utilities for moss crates.

mod language;
mod parsers;
mod paths;

pub use language::Language;
pub use parsers::Parsers;
pub use paths::get_moss_dir;

// Re-export grammar crates for use in other modules
pub use tree_sitter;
pub use tree_sitter_bash;
pub use tree_sitter_c;
pub use tree_sitter_cpp;
pub use tree_sitter_css;
pub use tree_sitter_go;
pub use tree_sitter_html;
pub use tree_sitter_java;
pub use tree_sitter_javascript;
pub use tree_sitter_json;
pub use tree_sitter_md;
pub use tree_sitter_python;
pub use tree_sitter_ruby;
pub use tree_sitter_rust;
pub use tree_sitter_toml_updated as tree_sitter_toml;
pub use tree_sitter_typescript;
pub use tree_sitter_yaml;

/// Symbol kind in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Import,
    Struct,
    Enum,
    Trait,
    Interface,
    Constant,
    Module,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Method => "method",
            SymbolKind::Variable => "variable",
            SymbolKind::Import => "import",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Constant => "constant",
            SymbolKind::Module => "module",
        }
    }
}
