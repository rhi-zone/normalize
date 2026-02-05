//! Core trait for language support.

use tree_sitter::Node;

// Re-export core types from normalize-facts-core
pub use normalize_facts_core::{
    Export, Import, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};

/// Embedded content block (e.g., JS in Vue, CSS in HTML)
#[derive(Debug, Clone)]
pub struct EmbeddedBlock {
    /// Grammar to use for parsing (e.g., "javascript", "css")
    pub grammar: &'static str,
    /// Extracted source content
    pub content: String,
    /// 1-indexed start line in the parent file
    pub start_line: usize,
}

// === Helper functions for common extractor patterns ===

/// Create a simple symbol with standard defaults.
///
/// Used by languages with straightforward function/method syntax where symbols:
/// - Have public visibility
/// - Use first line as signature
/// - Have no attributes or children
/// - Don't implement interfaces
///
/// Languages using this: cmake, glsl, graphql, hlsl, awk, elm, fish, haskell,
/// jq, julia, ocaml, powershell, zsh
pub fn simple_symbol(
    node: &tree_sitter::Node,
    content: &str,
    name: &str,
    kind: SymbolKind,
    docstring: Option<String>,
) -> Symbol {
    let text = &content[node.byte_range()];
    let first_line = text.lines().next().unwrap_or(text);

    Symbol {
        name: name.to_string(),
        kind,
        signature: first_line.trim().to_string(),
        docstring,
        attributes: Vec::new(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: Visibility::Public,
        children: Vec::new(),
        is_interface_impl: false,
        implements: Vec::new(),
    }
}

/// Create a simple function symbol (convenience wrapper).
pub fn simple_function_symbol(
    node: &tree_sitter::Node,
    content: &str,
    name: &str,
    docstring: Option<String>,
) -> Symbol {
    simple_symbol(node, content, name, SymbolKind::Function, docstring)
}

/// Unified language support trait.
///
/// Each language implements this trait to provide:
/// - Node kind classification
/// - Symbol extraction (functions, classes, types)
/// - Import/export parsing
/// - Complexity analysis nodes
/// - Visibility detection
/// - Edit support (container bodies, docstrings)
pub trait Language: Send + Sync {
    /// Display name for this language (e.g., "Python", "C++")
    fn name(&self) -> &'static str;

    /// File extensions this language handles (e.g., ["py", "pyi", "pyw"])
    fn extensions(&self) -> &'static [&'static str];

    /// Grammar name for arborium (e.g., "python", "rust")
    fn grammar_name(&self) -> &'static str;

    /// Whether this language has code symbols (functions, classes, etc.)
    fn has_symbols(&self) -> bool;

    // === Node Classification ===

    /// Container nodes that can hold methods (class, impl, module)
    fn container_kinds(&self) -> &'static [&'static str];

    /// Function/method definition nodes
    fn function_kinds(&self) -> &'static [&'static str];

    /// Type definition nodes (struct, enum, interface, type alias)
    fn type_kinds(&self) -> &'static [&'static str];

    /// Import statement nodes
    fn import_kinds(&self) -> &'static [&'static str];

    /// AST node kinds that may contain publicly visible symbols.
    /// For JS/TS: export_statement nodes.
    /// For Go/Java/Python: function/class/type declaration nodes.
    /// The extract_public_symbols() method filters by actual visibility.
    fn public_symbol_kinds(&self) -> &'static [&'static str];

    /// How this language determines symbol visibility
    fn visibility_mechanism(&self) -> VisibilityMechanism;

    // === Symbol Extraction ===

    /// Extract symbol from a function/method node
    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol>;

    /// Extract symbol from a container node (class, impl, module)
    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol>;

    /// Extract symbol from a type definition node
    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol>;

    /// Extract docstring/doc comment for a node
    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String>;

    /// Extract attributes/decorators for a node (e.g., #[test], @Test)
    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String>;

    // === Import/Export ===

    /// Extract imports from an import node (may return multiple)
    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import>;

    /// Format an import as source code.
    /// If `names` is Some, only include those names (for multi-import filtering).
    /// If `names` is None, format the complete import.
    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String;

    /// Extract public symbols from a node.
    /// The node is one of the kinds from public_symbol_kinds().
    /// For JS/TS: extracts exported names from export statements.
    /// For Go/Java/Python: checks visibility and returns public symbols.
    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export>;

    // === Scope Analysis ===

    /// Nodes that create new variable scopes (for scope analysis)
    /// Includes: loops, blocks, comprehensions, lambdas, with statements
    /// Note: Functions and containers (from function_kinds/container_kinds) also create scopes
    fn scope_creating_kinds(&self) -> &'static [&'static str];

    // === Control Flow ===

    /// Nodes that affect control flow (for CFG analysis)
    /// Includes: if, for, while, return, break, continue, try, match
    fn control_flow_kinds(&self) -> &'static [&'static str];

    // === Complexity ===

    /// Nodes that increase cyclomatic complexity
    fn complexity_nodes(&self) -> &'static [&'static str];

    /// Nodes that indicate nesting depth
    fn nesting_nodes(&self) -> &'static [&'static str];

    // === Display/Formatting ===

    /// Suffix to append to signatures for tree-sitter parsing.
    /// Function signatures are incomplete code fragments that need closing tokens
    /// to parse correctly (e.g., Rust `fn foo()` needs `{}`, Lua `function foo()` needs `end`).
    /// Returns the suffix to append, or empty string if none needed.
    fn signature_suffix(&self) -> &'static str;

    // === Visibility ===

    /// Check if a node is public/exported
    fn is_public(&self, node: &Node, content: &str) -> bool;

    /// Get visibility of a node
    fn get_visibility(&self, node: &Node, content: &str) -> Visibility;

    /// Check if a symbol is a test (for filtering).
    /// Each language must implement this - test conventions are language-specific.
    fn is_test_symbol(&self, symbol: &Symbol) -> bool;

    // === Embedded Languages ===

    /// Extract embedded content from a node (e.g., JS/CSS in Vue/HTML).
    /// Returns None for nodes that don't contain embedded code in another language.
    fn embedded_content(&self, node: &Node, content: &str) -> Option<EmbeddedBlock>;

    // === Edit Support ===

    /// Find the body node of a container (for prepend/append)
    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>>;

    /// Detect if first child of body is a docstring
    fn body_has_docstring(&self, body: &Node, content: &str) -> bool;

    // === Helpers ===

    /// Get the name of a node (typically via "name" field)
    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str>;
}
