//! Core trait for language support.

use tree_sitter::Node;

// Re-export core types from normalize-facts-core
pub use normalize_facts_core::{Import, Symbol, SymbolKind, Visibility};

/// Location of a container's body (for prepend/append editing operations)
#[derive(Debug)]
pub struct ContainerBody {
    /// Byte offset where body content starts (after opening delimiter/heading)
    pub content_start: usize,
    /// Byte offset where body content ends (before closing delimiter)
    pub content_end: usize,
    /// Indentation string for new content inserted into the body
    pub inner_indent: String,
    /// True if the body has no meaningful content (empty, only pass/braces)
    pub is_empty: bool,
}

/// Information about what a class/container implements or extends.
#[derive(Debug, Default)]
pub struct ImplementsInfo {
    /// True if this is an interface/protocol/trait definition (not a concrete class).
    pub is_interface: bool,
    /// List of implemented interfaces, superclasses, or mixed-in traits.
    pub implements: Vec<String>,
}

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

/// Capability trait: language has code symbols (functions, classes, types, etc.).
///
/// Config and data languages (CSS, HTML, JSON, TOML, XML, YAML) don't implement this.
/// All general-purpose programming languages do.
/// Access via `lang.as_symbols()` rather than `lang.has_symbols()`.
pub trait LanguageSymbols: Language {}

/// Capability trait: language can contain embedded blocks in another language.
///
/// Only a handful of multi-language formats implement this (Vue, HTML, Svelte).
/// Access via `lang.as_embedded()` rather than casting.
pub trait LanguageEmbedded: Language {
    /// Extract embedded content from a node (e.g., JS/CSS in Vue/HTML).
    /// Returns None for nodes that don't contain embedded code in another language.
    fn embedded_content(&self, node: &Node, content: &str) -> Option<EmbeddedBlock>;
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

    /// Capability query: returns `Some(self)` if this language has code symbols
    /// (functions, classes, types, etc.). Returns `None` for config/data languages.
    /// Implement `LanguageSymbols` and override this to opt in.
    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        None
    }

    // === Symbol Building ===

    /// Extract the docstring for a definition node.
    /// Called by generic extraction for every tagged symbol.
    /// Returns None if this language has no docstring convention or the node has no docstring.
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    /// Extract attributes/annotations/decorators attached to a definition node.
    /// Called by generic extraction for every tagged symbol.
    /// Returns empty vec if this language has no attribute convention.
    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    /// Extract interfaces/traits/superclasses that a container node implements/extends.
    /// Called by generic extraction for container nodes only.
    fn extract_implements(&self, _node: &Node, _content: &str) -> ImplementsInfo {
        ImplementsInfo::default()
    }

    /// Build the display signature for a definition node.
    /// Default: first line of the node's source text (trimmed).
    /// Override for languages where first-line is incomplete (e.g. Rust, Go, Java).
    fn build_signature(&self, node: &Node, content: &str) -> String {
        let text = &content[node.byte_range()];
        text.lines().next().unwrap_or(text).trim().to_string()
    }

    /// Refine the symbol kind for a tagged node.
    /// Called after tag classification assigns an initial kind (e.g. `definition.class` → `Class`).
    /// Languages can override this to return a more specific kind based on the node's concrete type.
    /// Default: return `tag_kind` unchanged.
    fn refine_kind(&self, node: &Node, _content: &str, tag_kind: SymbolKind) -> SymbolKind {
        let _ = node;
        tag_kind
    }

    // === Import/Export ===

    /// Extract imports from an import node (may return multiple)
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    /// Format an import as source code.
    /// If `names` is Some, only include those names (for multi-import filtering).
    /// If `names` is None, format the complete import.
    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        String::new()
    }

    // === Display/Formatting ===

    /// Suffix to append to signatures for tree-sitter parsing.
    /// Function signatures are incomplete code fragments that need closing tokens
    /// to parse correctly (e.g., Rust `fn foo()` needs `{}`, Lua `function foo()` needs `end`).
    /// Returns the suffix to append, or empty string if none needed.
    fn signature_suffix(&self) -> &'static str {
        ""
    }

    // === Visibility ===

    /// Get visibility of a node.
    ///
    /// This is a genuine interface method (not just an impl helper): `normalize-deps`
    /// calls it externally during export detection to decide which tagged nodes are
    /// public. The alternative — calling `extract_function/container/type()` and
    /// inspecting `symbol.visibility` — would be correct but unnecessarily heavy
    /// (computes signature, docstring, etc. just to check one field).
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    /// Check if a symbol is a test (for filtering).
    fn is_test_symbol(&self, _symbol: &Symbol) -> bool {
        false
    }

    /// Glob patterns (relative, using `**` wildcards) that identify dedicated test files.
    /// Used to build a GlobSet for fast batch matching.
    /// Return `&[]` for languages with no dedicated test files (e.g. those using only inline tests).
    fn test_file_globs(&self) -> &'static [&'static str] {
        &[]
    }

    /// Capability query: returns `Some(self)` if this language can contain embedded blocks
    /// in another language (e.g., JS in Vue, CSS in HTML). Returns `None` for most languages.
    /// Implement `LanguageEmbedded` and override this to opt in.
    fn as_embedded(&self) -> Option<&dyn LanguageEmbedded> {
        None
    }

    // === Edit Support ===

    /// Find the body node of a container (for prepend/append)
    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }

    /// Detect if first child of body is a docstring
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    /// Analyze a container body node and return the editable byte range.
    /// `body_node` is the node returned by `container_body`.
    /// Returns None if this language doesn't support container body editing.
    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    // === Helpers ===

    /// Get the name of a node (typically via "name" field)
    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
    }
}
