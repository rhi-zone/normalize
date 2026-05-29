//! Core trait for language support.

use std::path::{Path, PathBuf};
use tree_sitter::Node;

// Re-export core types from normalize-facts-core
pub use normalize_facts_core::{Import, InterfaceResolver, Symbol, SymbolKind, Visibility};

/// Configuration discovered from workspace manifests for module resolution.
pub struct ResolverConfig {
    /// Workspace root used for relative path anchoring.
    pub workspace_root: PathBuf,
    /// Language-specific path mappings (e.g. tsconfig paths, Cargo workspace members).
    pub path_mappings: Vec<(String, PathBuf)>,
    /// Additional search roots (e.g. PYTHONPATH entries, Go module cache).
    pub search_roots: Vec<PathBuf>,
}

/// A parsed import specifier.
pub struct ImportSpec {
    /// Raw specifier string (e.g. "std::collections::HashMap", "./utils", "numpy").
    pub raw: String,
    /// Whether this is a relative import (starts with ./ or ../).
    pub is_relative: bool,
    /// The imported names, if specified (e.g. `use foo::{bar, baz}` → ["bar", "baz"]).
    /// Empty for glob/wildcard imports.
    pub names: Vec<String>,
    /// True if this is a glob/wildcard import (e.g. `use foo::*`, `from x import *`).
    pub is_glob: bool,
}

/// A resolved module identifier.
pub struct ModuleId {
    pub canonical_path: String,
}

/// Result of resolving an import specifier to a file.
pub enum Resolution {
    /// Resolved to exactly one file + exported name.
    Resolved(PathBuf, String),
    /// Multiple possible resolutions (ambiguous).
    Ambiguous(Vec<(PathBuf, String)>),
    /// Could not resolve.
    NotFound,
    /// This language has no module system; resolution is not applicable.
    NotApplicable,
}

/// Per-language module resolver.
///
/// Implements the Rust/TS/Python/etc-specific logic for turning an import
/// specifier into a resolved file path.
pub trait ModuleResolver: Send + Sync {
    /// Read workspace config from the given root (e.g. Cargo.toml, tsconfig.json).
    fn workspace_config(&self, root: &Path) -> ResolverConfig;
    /// Return the canonical module identity/ies of a file within the workspace.
    fn module_of_file(&self, root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId>;
    /// Resolve an import specifier from `from_file` to a target file + name.
    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution;
}

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

    /// Post-process the symbol list after all symbols have been extracted from a file.
    ///
    /// Called once per file, after the tags-based extraction pass. Use this for
    /// language-specific structural transformations that cannot be expressed in a
    /// `.scm` query:
    /// - Rust: fold impl-block methods into their owning struct/enum.
    /// - Haskell: collapse multi-equation function definitions into a single symbol.
    /// - TypeScript/JavaScript: mark methods that satisfy interface contracts.
    ///
    /// The `resolver` and `current_file` parameters are provided for languages
    /// that need cross-file lookups (e.g. TypeScript interface resolution). Most
    /// implementations can ignore them.
    ///
    /// Default: no-op.
    fn post_process_symbols(
        &self,
        _symbols: &mut Vec<Symbol>,
        _resolver: Option<&dyn InterfaceResolver>,
        _current_file: &str,
    ) {
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

    // === Module-level documentation ===

    /// Extract the module-level doc comment from raw file source.
    ///
    /// Called when viewing a file (not a specific symbol) to populate `ViewReport.summary`.
    /// Returns `None` if this language has no module-doc convention or the file has none.
    ///
    /// Conventions by language:
    /// - Rust: leading `//!` inner-doc comment lines
    /// - Python: first statement is a string literal (`"""..."""`)
    /// - Go: line comment(s) immediately before `package foo`
    /// - JavaScript/TypeScript: leading `/** ... */` block comment at top of file
    /// - Ruby: leading `#` comment block (ignoring `# frozen_string_literal` lines)
    fn extract_module_doc(&self, _src: &str) -> Option<String> {
        None
    }

    // === Module Resolution ===

    /// Return the module resolver for this language, if it has one.
    ///
    /// Languages with a module system (Rust, TypeScript, Python, Go, etc.) implement
    /// this to enable cross-file name resolution. Languages without a module system
    /// (Bash, GLSL, etc.) return `None`.
    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        None
    }

    // === Helpers ===

    /// Get the name of a node (typically via "name" field)
    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
    }

    /// Is this node a genuine definition of a symbol (vs. an import/re-export/alias)?
    ///
    /// Several grammars give import/re-export nodes a `name` field that
    /// [`Language::node_name`] happily returns — e.g. Python's
    /// `from .sessions import Session` is an `import_from_statement` whose `name`
    /// is `Session`, even though the real `class Session` lives elsewhere. Consumers
    /// that locate a symbol by name (e.g. source-tree doc extraction) use this to
    /// prefer the actual definition over a re-export that merely shadows the name.
    ///
    /// Default: `true` (a name-bearing node is treated as a definition). Languages
    /// whose grammar attaches names to import/re-export nodes override this to return
    /// `false` for those node kinds.
    fn is_definition_node(&self, _node: &Node) -> bool {
        true
    }
}
