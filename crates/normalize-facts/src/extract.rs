//! Shared symbol extraction from source code.
//!
//! This module provides the core AST traversal logic for extracting
//! symbols, imports, and other facts from source files.
//!
//! ## Extraction paths
//!
//! Symbol extraction uses the tags path: a `*.tags.scm` query is run against the
//! tree-sitter parse tree. The query groups `@definition.*` + `@name` captures per
//! match, reconstructs nesting by line-range containment, and returns a `Vec<Symbol>`.
//!
//! After extraction, post-processing steps apply (Rust impl-block merging,
//! TypeScript/JavaScript interface marking).

use crate::ca_cache;
use crate::parsers;
use normalize_facts_core::InterfaceResolver;
use normalize_facts_core::SymbolKind;
use normalize_languages::{Language, Symbol, Visibility, support_for_path};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter;

/// Result of extracting symbols from a file.
pub struct ExtractResult {
    /// Top-level symbols (nested structure preserved)
    pub symbols: Vec<Symbol>,
    /// File path for context
    pub file_path: String,
}

impl ExtractResult {
    /// Filter to only type definitions (class, struct, enum, trait, interface)
    /// Returns a new ExtractResult with only type-like symbols, and strips methods from classes
    pub fn filter_types(&self) -> ExtractResult {
        use normalize_facts_core::SymbolKind;

        fn is_type_kind(kind: SymbolKind) -> bool {
            matches!(
                kind,
                SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Enum
                    | SymbolKind::Trait
                    | SymbolKind::Interface
                    | SymbolKind::Type
                    | SymbolKind::Module
            )
        }

        fn filter_symbol(sym: &Symbol) -> Option<Symbol> {
            if is_type_kind(sym.kind) {
                // For types, keep only nested types (not methods)
                let type_children: Vec<_> = sym.children.iter().filter_map(filter_symbol).collect();
                Some(Symbol {
                    name: sym.name.clone(),
                    kind: sym.kind,
                    signature: sym.signature.clone(),
                    docstring: sym.docstring.clone(),
                    attributes: Vec::new(),
                    start_line: sym.start_line,
                    end_line: sym.end_line,
                    visibility: sym.visibility,
                    children: type_children,
                    is_interface_impl: sym.is_interface_impl,
                    implements: sym.implements.clone(),
                    complexity: sym.complexity,
                })
            } else {
                None
            }
        }

        let filtered_symbols: Vec<_> = self.symbols.iter().filter_map(filter_symbol).collect();

        ExtractResult {
            symbols: filtered_symbols,
            file_path: self.file_path.clone(),
        }
    }

    /// Filter out test functions and test modules.
    /// Uses Language::is_test_symbol() for language-specific detection.
    pub fn filter_tests(&self) -> ExtractResult {
        use normalize_languages::support_for_path;
        use std::path::Path;

        let lang = support_for_path(Path::new(&self.file_path));

        fn filter_symbol(sym: &Symbol, lang: Option<&dyn Language>) -> Option<Symbol> {
            let is_test = match lang {
                Some(l) => l.is_test_symbol(sym),
                None => false, // Unknown language: keep everything
            };
            if is_test {
                return None;
            }
            let filtered_children: Vec<_> = sym
                .children
                .iter()
                .filter_map(|c| filter_symbol(c, lang))
                .collect();
            Some(Symbol {
                name: sym.name.clone(),
                kind: sym.kind,
                signature: sym.signature.clone(),
                docstring: sym.docstring.clone(),
                attributes: sym.attributes.clone(),
                start_line: sym.start_line,
                end_line: sym.end_line,
                visibility: sym.visibility,
                children: filtered_children,
                is_interface_impl: sym.is_interface_impl,
                implements: sym.implements.clone(),
                complexity: sym.complexity,
            })
        }

        let lang_ref: Option<&dyn Language> = lang.map(|l| l as &dyn Language);
        let filtered_symbols: Vec<_> = self
            .symbols
            .iter()
            .filter_map(|s| filter_symbol(s, lang_ref))
            .collect();

        ExtractResult {
            symbols: filtered_symbols,
            file_path: self.file_path.clone(),
        }
    }
}

/// Options for symbol extraction.
#[derive(Clone)]
pub struct ExtractOptions {
    /// Include private/non-public symbols (default: true for code exploration)
    pub include_private: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            // Default to including all symbols - normalize is for code exploration,
            // not API documentation. This ensures trait impl methods are visible.
            include_private: true,
        }
    }
}

/// Resolver that parses files on-demand for cross-file interface lookups.
/// This is the fallback when no index is available.
pub struct OnDemandResolver<'a> {
    root: &'a std::path::Path,
}

impl<'a> OnDemandResolver<'a> {
    pub fn new(root: &'a std::path::Path) -> Self {
        Self { root }
    }
}

impl InterfaceResolver for OnDemandResolver<'_> {
    fn resolve_interface_methods(&self, name: &str, current_file: &str) -> Option<Vec<String>> {
        use normalize_languages::support_for_path;

        let current_path = std::path::Path::new(current_file);
        let current_dir = current_path.parent()?;

        // Try common patterns for interface files
        // This is a heuristic - we check nearby files that might contain the interface
        let candidates = [
            "types.ts",
            "interfaces.ts",
            "index.ts",
            "../types.ts",
            "../interfaces.ts",
            "../index.ts",
        ];

        for candidate in candidates {
            let candidate_path = if candidate.starts_with("..") {
                current_dir.parent()?.join(&candidate[3..])
            } else {
                current_dir.join(candidate)
            };

            // Try with root prefix
            let full_path = self.root.join(&candidate_path);
            if !full_path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&full_path).ok()?;
            // Verify it's a supported file type
            let _support = support_for_path(&full_path)?;

            // Parse the file and look for the interface
            let extractor = Extractor::new();
            // Don't use resolver here to avoid recursion
            let result = extractor.extract(&full_path, &content);

            for sym in &result.symbols {
                if sym.name == name
                    && matches!(
                        sym.kind,
                        normalize_languages::SymbolKind::Interface
                            | normalize_languages::SymbolKind::Class
                    )
                {
                    let methods: Vec<String> = sym
                        .children
                        .iter()
                        .filter(|c| {
                            matches!(
                                c.kind,
                                normalize_languages::SymbolKind::Method
                                    | normalize_languages::SymbolKind::Function
                            )
                        })
                        .map(|c| c.name.clone())
                        .collect();
                    if !methods.is_empty() {
                        return Some(methods);
                    }
                }
            }
        }

        None
    }
}

/// Shared symbol extractor using the Language trait.
pub struct Extractor {
    options: ExtractOptions,
}

impl Default for Extractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor {
    pub fn new() -> Self {
        Self {
            options: ExtractOptions::default(),
        }
    }

    pub fn with_options(options: ExtractOptions) -> Self {
        Self { options }
    }

    /// Extract symbols from a file.
    pub fn extract(&self, path: &Path, content: &str) -> ExtractResult {
        self.extract_with_resolver(path, content, None)
    }

    /// Extract symbols from a file with optional cross-file interface resolution.
    pub fn extract_with_resolver(
        &self,
        path: &Path,
        content: &str,
        resolver: Option<&dyn InterfaceResolver>,
    ) -> ExtractResult {
        let file_path = path.to_string_lossy().to_string();
        let symbols = match support_for_path(path) {
            Some(support) => self.extract_with_support(content, support, resolver, &file_path),
            None => Vec::new(),
        };

        ExtractResult { symbols, file_path }
    }

    fn extract_with_support(
        &self,
        content: &str,
        support: &dyn Language,
        resolver: Option<&dyn InterfaceResolver>,
        current_file: &str,
    ) -> Vec<Symbol> {
        let grammar_name = support.grammar_name();

        // Cache version encodes the extraction schema and include_private flag.
        // Bump this string whenever the Symbol struct, post-processing, or query
        // semantics change in a way that invalidates existing cached results.
        // Cross-file resolver results are not cached (resolver.is_none() guard below).
        // v2 (2026-07-15): Symbol gained a `complexity` field.
        let cache_ver = if self.options.include_private {
            "symbols-v2-all"
        } else {
            "symbols-v2-public"
        };

        // Check the persistent symbol cache before parsing (only when no cross-file
        // resolver is involved, as resolver results depend on other files).
        if resolver.is_none()
            && let Some(cache) = ca_cache::symbol_cache()
        {
            let hash = blake3::hash(content.as_bytes());
            match cache.get::<Vec<Symbol>>(hash.as_bytes(), cache_ver, grammar_name) {
                Ok(Some(cached)) => return cached,
                Ok(None) => {} // cache miss — fall through to parse
                Err(e) => {
                    tracing::debug!(
                        "normalize-facts: symbol cache get error for {}: {}",
                        current_file,
                        e
                    );
                }
            }
        }

        let tree = match parsers::parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        // Use the tags-based extraction path with cached compiled queries.
        let loader = parsers::grammar_loader();
        let mut symbols = if let Some(tags_query_str) = loader.get_tags(grammar_name) {
            loader
                .get_compiled_query(grammar_name, "tags", &tags_query_str)
                .and_then(|query| {
                    collect_symbols_from_tags(
                        &tree,
                        &query,
                        content,
                        support,
                        self.options.include_private,
                    )
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Language-specific post-processing: fold impl blocks, dedup multi-equation
        // definitions, mark interface implementations, etc.
        support.post_process_symbols(&mut symbols, resolver, current_file);

        // Store in the persistent symbol cache (only when no cross-file resolver
        // was used, so the result is fully content-addressed).
        if resolver.is_none()
            && let Some(cache) = ca_cache::symbol_cache()
        {
            let hash = blake3::hash(content.as_bytes());
            if let Err(e) = cache.put(hash.as_bytes(), cache_ver, grammar_name, &symbols) {
                tracing::debug!(
                    "normalize-facts: symbol cache put error for {}: {}",
                    current_file,
                    e
                );
            }
        }

        symbols
    }
}

/// Map a `@definition.*` capture name to a `SymbolKind`.
///
/// Returns `None` for capture names that are not definitions (e.g., `reference.call`),
/// which should be ignored during symbol extraction.
fn tags_capture_to_kind(capture_name: &str) -> Option<SymbolKind> {
    match capture_name {
        "definition.function" => Some(SymbolKind::Function),
        // Methods are tagged as Function here; they get re-classified to Method
        // once we reconstruct nesting (children of containers become methods).
        "definition.method" => Some(SymbolKind::Function),
        "definition.class" => Some(SymbolKind::Class),
        "definition.interface" => Some(SymbolKind::Interface),
        "definition.module" => Some(SymbolKind::Module),
        "definition.type" => Some(SymbolKind::Type),
        "definition.enum" => Some(SymbolKind::Enum),
        "definition.heading" => Some(SymbolKind::Heading),
        // No Macro variant — map to Function (closest semantic equivalent)
        "definition.macro" => Some(SymbolKind::Function),
        "definition.constant" => Some(SymbolKind::Constant),
        "definition.var" => Some(SymbolKind::Variable),
        _ => None,
    }
}

/// Whether a `SymbolKind` is a container that can hold child symbols.
fn is_container_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Module
            | SymbolKind::Enum
            | SymbolKind::Heading
    )
}

/// Intermediate record built from a single tags-query match before nesting reconstruction.
/// Retains the node ID so we can call Language trait methods on the correct node.
struct TagDef<'tree> {
    /// The definition AST node (e.g. function_item, class_definition).
    node: tree_sitter::Node<'tree>,
    /// `SymbolKind` derived from the `@definition.*` capture name.
    kind: SymbolKind,
    /// True when the capture name was `definition.method` (explicit method tag).
    is_method_capture: bool,
    /// True when the capture name identifies a container kind (class/interface/module).
    is_container: bool,
    /// Line numbers (1-indexed) of the definition node.
    start_line: usize,
    end_line: usize,
}

/// Build a `Symbol` from a single `TagDef` using the Language semantic hooks.
fn build_symbol_from_def<'tree>(
    def: &TagDef<'tree>,
    content: &str,
    support: &dyn Language,
    in_container: bool,
) -> Option<Symbol> {
    let name = support.node_name(&def.node, content)?;
    let tag_kind = support.refine_kind(&def.node, content, def.kind);
    let kind =
        if def.is_method_capture || (in_container && matches!(tag_kind, SymbolKind::Function)) {
            SymbolKind::Method
        } else {
            tag_kind
        };
    let implements_info = if def.is_container {
        support.extract_implements(&def.node, content)
    } else {
        normalize_languages::ImplementsInfo::default()
    };
    // Cyclomatic complexity only applies to functions/methods — other symbol kinds
    // (classes, modules, types, ...) genuinely have no complexity concept.
    let complexity = matches!(kind, SymbolKind::Function | SymbolKind::Method)
        .then(|| compute_complexity(&def.node, support, content.as_bytes()));
    Some(Symbol {
        name: name.to_string(),
        kind,
        signature: support.build_signature(&def.node, content),
        docstring: support.extract_docstring(&def.node, content),
        attributes: support.extract_attributes(&def.node, content),
        start_line: def.node.start_position().row + 1,
        end_line: def.node.end_position().row + 1,
        visibility: support.get_visibility(&def.node, content),
        children: Vec::new(),
        is_interface_impl: implements_info.is_interface,
        implements: implements_info.implements,
        complexity,
    })
}

/// Extract symbols from a parsed tree using a tags query.
///
/// Uses the tags query for *node classification* (which nodes are which kind of def),
/// then calls the Language semantic hooks on those nodes for symbol content
/// (name, signature, visibility, docstring, implements, attributes, etc.).
///
/// Nesting is reconstructed by line-range containment: a def whose line range is
/// fully enclosed by a container def is placed as a child of that container.
///
/// Returns `None` if the query produces no definition matches (caller falls back to
/// the trait path).
fn collect_symbols_from_tags<'tree>(
    tree: &'tree tree_sitter::Tree,
    query: &tree_sitter::Query,
    content: &str,
    support: &dyn Language,
    include_private: bool,
) -> Option<Vec<Symbol>> {
    let capture_names = query.capture_names();

    // We require a @name capture to be present in the query.
    let name_idx = capture_names.iter().position(|n| *n == "name")?;
    let _ = name_idx; // present but not needed — definition node gives us position

    // Run the query and collect TagDef records.
    let root = tree.root_node();
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(query, root, content.as_bytes());

    let mut defs: Vec<TagDef<'tree>> = Vec::new();

    while let Some(m) = matches.next() {
        // Each match should contain a @definition.* capture.
        // We skip matches that have no definition capture (e.g. pure reference matches).
        let mut def_capture: Option<(tree_sitter::Node<'tree>, &str)> = None;

        for capture in m.captures {
            let cn = &capture_names[capture.index as usize];
            if tags_capture_to_kind(cn).is_some() {
                // SAFETY: The tree lives as long as 'tree; captures borrow from it.
                let node = capture.node;
                def_capture = Some((node, cn));
            }
        }

        let Some((def_node, capture_name)) = def_capture else {
            continue;
        };
        let kind = match tags_capture_to_kind(capture_name) {
            Some(k) => k,
            None => continue,
        };

        // Apply language-specific kind refinement before determining container status,
        // so languages like JSON can promote Variable → Module for object-valued pairs.
        let refined_kind = support.refine_kind(&def_node, content, kind);
        defs.push(TagDef {
            node: def_node,
            kind,
            is_method_capture: capture_name == "definition.method",
            is_container: is_container_kind(refined_kind),
            start_line: def_node.start_position().row + 1,
            end_line: def_node.end_position().row + 1,
        });
    }

    if defs.is_empty() {
        return None;
    }

    // Sort by start line, with outer containers before inner defs at the same line.
    defs.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(b.end_line.cmp(&a.end_line))
    });

    // De-duplicate: remove defs with identical byte ranges.
    // Some tags queries match the same node multiple times (e.g. both a generic and
    // a specific pattern). Keep the first (which has the most specific kind after sorting).
    defs.dedup_by(|b, a| {
        a.node.start_byte() == b.node.start_byte() && a.node.end_byte() == b.node.end_byte()
    });

    // Container indices (for nesting reconstruction).
    let container_idxs: Vec<usize> = (0..defs.len()).filter(|&i| defs[i].is_container).collect();

    // Two-phase assembly: first build all symbols with parent info, then assemble tree.
    // This supports arbitrary nesting depth (namespaces > classes > methods, or
    // deeply nested data format keys).

    // Phase 1: Build symbols and record parent relationships.
    // symbols[i] corresponds to defs[i] (None if skipped due to visibility).
    let mut symbols: Vec<Option<Symbol>> = Vec::with_capacity(defs.len());
    let mut parent_of: Vec<Option<usize>> = Vec::with_capacity(defs.len()); // def_idx → parent def_idx

    for i in 0..defs.len() {
        let def = &defs[i];

        let enclosing_ci = container_idxs
            .iter()
            .filter(|&&ci| ci != i)
            .rev()
            .find(|&&ci| {
                let c = &defs[ci];
                c.start_line <= def.start_line && c.end_line >= def.end_line
            });

        let in_container = enclosing_ci.is_some();

        let Some(mut sym) = build_symbol_from_def(def, content, support, in_container) else {
            symbols.push(None);
            parent_of.push(None);
            continue;
        };

        if !include_private
            && matches!(
                sym.visibility,
                Visibility::Private | Visibility::Protected | Visibility::Internal
            )
        {
            symbols.push(None);
            parent_of.push(None);
            continue;
        }

        if def.is_container {
            sym.children.clear();
        }

        symbols.push(Some(sym));
        parent_of.push(enclosing_ci.copied());
    }

    // Phase 2: Assemble tree bottom-up. Process in reverse order so children are
    // moved into their parents before the parent is moved into its parent.
    // We use Vec<Option<Symbol>> so we can take ownership via .take().
    for i in (0..symbols.len()).rev() {
        if let Some(pi) = parent_of[i]
            && symbols[pi].is_some()
            && symbols[i].is_some()
        {
            let child = symbols[i].take().unwrap();
            symbols[pi].as_mut().unwrap().children.push(child);
        }
    }

    // Collect remaining top-level symbols (those not moved into a parent).
    // Reverse children since we assembled bottom-up.
    let mut top_level: Vec<Symbol> = Vec::new();
    for sym_opt in &mut symbols {
        if let Some(mut sym) = sym_opt.take() {
            sym.children.reverse();
            reverse_children_recursive(&mut sym.children);
            top_level.push(sym);
        }
    }

    if top_level.is_empty() {
        None
    } else {
        Some(top_level)
    }
}

/// Recursively reverse children vectors (needed because bottom-up assembly reverses order).
fn reverse_children_recursive(children: &mut [Symbol]) {
    for child in children.iter_mut() {
        child.children.reverse();
        reverse_children_recursive(&mut child.children);
    }
}

/// Compute cyclomatic complexity for a function node using the `.complexity.scm` query.
/// Returns 1 (base complexity) for languages without a complexity query.
pub fn compute_complexity(
    node: &tree_sitter::Node,
    support: &dyn Language,
    source: &[u8],
) -> usize {
    let grammar_name = support.grammar_name();
    let loader = parsers::grammar_loader();
    if let Some(scm) = loader.get_complexity(grammar_name)
        && let Some(query) = loader.get_compiled_query(grammar_name, "complexity", &scm)
    {
        return count_complexity_with_query(node, source, &query);
    }
    1
}

/// Count complexity using a `@complexity` query.
fn count_complexity_with_query(
    node: &tree_sitter::Node,
    source: &[u8],
    query: &tree_sitter::Query,
) -> usize {
    let complexity_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "complexity");
    let Some(complexity_idx) = complexity_idx else {
        return 1;
    };

    let mut qcursor = tree_sitter::QueryCursor::new();
    qcursor.set_byte_range(node.byte_range());
    let mut complexity = 1usize;
    let mut matches = qcursor.matches(query, *node, source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index as usize == complexity_idx {
                complexity += 1;
            }
        }
    }
    complexity
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_python() {
        let extractor = Extractor::new();
        let content = r#"
def foo(x: int) -> str:
    """Convert int to string."""
    return str(x)

class Bar:
    """A bar class."""
    def method(self):
        pass
"#;
        let result = extractor.extract(&PathBuf::from("test.py"), content);
        assert_eq!(result.symbols.len(), 2);

        let foo = &result.symbols[0];
        assert_eq!(foo.name, "foo");
        assert!(foo.signature.contains("def foo"));
        assert_eq!(foo.docstring.as_deref(), Some("Convert int to string."));

        let bar = &result.symbols[1];
        assert_eq!(bar.name, "Bar");
        assert_eq!(bar.children.len(), 1);
        assert_eq!(bar.children[0].name, "method");
    }

    #[test]
    fn test_extract_rust() {
        let extractor = Extractor::new();
        let content = r#"
/// A simple struct
pub struct Foo {
    x: i32,
}

impl Foo {
    /// Create a new Foo
    pub fn new(x: i32) -> Self {
        Self { x }
    }
}
"#;
        let result = extractor.extract(&PathBuf::from("test.rs"), content);

        // Should have struct with method from impl merged
        let foo = result.symbols.iter().find(|s| s.name == "Foo").unwrap();
        assert!(foo.signature.contains("pub struct Foo"));
        assert_eq!(foo.children.len(), 1);
        assert_eq!(foo.children[0].name, "new");
    }

    #[test]
    fn test_include_private() {
        let extractor = Extractor::with_options(ExtractOptions {
            include_private: true,
        });
        let content = r#"
fn private_fn() {}
pub fn public_fn() {}
"#;
        let result = extractor.extract(&PathBuf::from("test.rs"), content);
        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"private_fn"));
        assert!(names.contains(&"public_fn"));
    }

    #[test]
    fn test_typescript_interface_impl_detection() {
        let extractor = Extractor::new();
        let content = r#"
interface IFoo {
  bar(): void;
  baz(): number;
}

class Foo implements IFoo {
  bar() {}
  baz() { return 1; }
  other() {}
}
"#;
        let result = extractor.extract(&PathBuf::from("test.ts"), content);

        // Should have interface and class
        assert_eq!(result.symbols.len(), 2);

        let interface = &result.symbols[0];
        assert_eq!(interface.name, "IFoo");
        assert_eq!(interface.children.len(), 2); // bar, baz

        let class = &result.symbols[1];
        assert_eq!(class.name, "Foo");
        assert_eq!(class.implements, vec!["IFoo"]);
        assert_eq!(class.children.len(), 3); // bar, baz, other

        // bar and baz should be marked as interface implementations
        let bar = class.children.iter().find(|c| c.name == "bar").unwrap();
        let baz = class.children.iter().find(|c| c.name == "baz").unwrap();
        let other = class.children.iter().find(|c| c.name == "other").unwrap();

        assert!(
            bar.is_interface_impl,
            "bar should be marked as interface impl"
        );
        assert!(
            baz.is_interface_impl,
            "baz should be marked as interface impl"
        );
        assert!(
            !other.is_interface_impl,
            "other should NOT be marked as interface impl"
        );
    }

    #[test]
    fn test_cross_file_interface_impl_with_mock_resolver() {
        // Mock resolver that returns methods for IRemote interface
        struct MockResolver;
        impl InterfaceResolver for MockResolver {
            fn resolve_interface_methods(
                &self,
                name: &str,
                _current_file: &str,
            ) -> Option<Vec<String>> {
                if name == "IRemote" {
                    Some(vec![
                        "remoteMethod".to_string(),
                        "anotherRemote".to_string(),
                    ])
                } else {
                    None
                }
            }
        }

        let extractor = Extractor::new();
        // Class implements IRemote which is NOT in this file
        let content = r#"
class Foo implements IRemote {
  remoteMethod() {}
  anotherRemote() { return 1; }
  localMethod() {}
}
"#;
        let resolver = MockResolver;
        let result =
            extractor.extract_with_resolver(&PathBuf::from("test.ts"), content, Some(&resolver));

        assert_eq!(result.symbols.len(), 1);

        let class = &result.symbols[0];
        assert_eq!(class.name, "Foo");
        assert_eq!(class.implements, vec!["IRemote"]);
        assert_eq!(class.children.len(), 3);

        // remoteMethod and anotherRemote should be marked as interface implementations
        let remote_method = class
            .children
            .iter()
            .find(|c| c.name == "remoteMethod")
            .unwrap();
        let another_remote = class
            .children
            .iter()
            .find(|c| c.name == "anotherRemote")
            .unwrap();
        let local_method = class
            .children
            .iter()
            .find(|c| c.name == "localMethod")
            .unwrap();

        assert!(
            remote_method.is_interface_impl,
            "remoteMethod should be marked as interface impl"
        );
        assert!(
            another_remote.is_interface_impl,
            "anotherRemote should be marked as interface impl"
        );
        assert!(
            !local_method.is_interface_impl,
            "localMethod should NOT be marked as interface impl"
        );
    }

    #[test]
    fn test_cross_file_resolver_not_found() {
        // Mock resolver that returns None (interface not found)
        struct NotFoundResolver;
        impl InterfaceResolver for NotFoundResolver {
            fn resolve_interface_methods(
                &self,
                _name: &str,
                _current_file: &str,
            ) -> Option<Vec<String>> {
                None
            }
        }

        let extractor = Extractor::new();
        let content = r#"
class Foo implements IUnknown {
  someMethod() {}
}
"#;
        let resolver = NotFoundResolver;
        let result =
            extractor.extract_with_resolver(&PathBuf::from("test.ts"), content, Some(&resolver));

        let class = &result.symbols[0];
        // When interface is not found, methods should NOT be marked as interface impl
        let some_method = class
            .children
            .iter()
            .find(|c| c.name == "someMethod")
            .unwrap();
        assert!(
            !some_method.is_interface_impl,
            "someMethod should NOT be marked when interface not found"
        );
    }

    // -- implements extraction tests across languages --

    /// Returns `None` if no grammar is available for `file`'s extension — tests
    /// then early-return cleanly instead of asserting on an empty extraction.
    /// `Some(vec)` may still be empty if the file has no `implements` symbols.
    ///
    /// Run `cargo xtask build-grammars` and set
    /// `NORMALIZE_GRAMMAR_PATH=target/grammars` to enable all language tests.
    fn extract_implements(file: &str, code: &str) -> Option<Vec<(String, Vec<String>)>> {
        // Probe whether the underlying grammar is loadable before extracting.
        // `support_for_path` checks our Language registry; `parse_with_grammar`
        // ensures the .so is actually present on disk.
        let path = PathBuf::from(file);
        let support = normalize_languages::support_for_path(&path)?;
        let grammar = support.grammar_name();
        parsers::parse_with_grammar(grammar, code)?;
        let extractor = Extractor::new();
        let result = extractor.extract(&path, code);
        fn collect(symbols: &[normalize_languages::Symbol]) -> Vec<(String, Vec<String>)> {
            let mut out = Vec::new();
            for s in symbols {
                if !s.implements.is_empty() {
                    out.push((s.name.clone(), s.implements.clone()));
                }
                out.extend(collect(&s.children));
            }
            out
        }
        Some(collect(&result.symbols))
    }

    #[test]
    fn test_implements_python() {
        let Some(results) = extract_implements("test.py", "class Foo(Bar, Baz):\n    pass\n")
        else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_rust() {
        let Some(results) = extract_implements(
            "test.rs",
            "pub trait MyTrait {}\npub struct Foo;\nimpl MyTrait for Foo {}\n",
        ) else {
            return;
        };
        let impl_sym = results.iter().find(|(n, _)| n == "Foo").unwrap();
        assert_eq!(impl_sym.1, vec!["MyTrait"]);
    }

    #[test]
    fn test_implements_java() {
        let Some(results) = extract_implements(
            "test.java",
            "class Foo extends Bar implements Baz, Qux {}\n",
        ) else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into(), "Qux".into()])]
        );
    }

    #[test]
    fn test_implements_javascript() {
        let Some(results) = extract_implements("test.js", "class Foo extends Bar {}\n") else {
            return;
        };
        assert_eq!(results, vec![("Foo".into(), vec!["Bar".into()])]);
    }

    #[test]
    fn test_implements_typescript() {
        let Some(results) =
            extract_implements("test.ts", "class Foo extends Bar implements Baz {}\n")
        else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_cpp() {
        let Some(results) = extract_implements(
            "test.cpp",
            "class Derived : public Base, public Other {};\n",
        ) else {
            return;
        };
        assert_eq!(
            results,
            vec![("Derived".into(), vec!["Base".into(), "Other".into()])]
        );
    }

    #[test]
    fn test_implements_scala() {
        let Some(results) = extract_implements("test.scala", "class Foo extends Bar with Baz {}\n")
        else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_ruby() {
        let Some(results) = extract_implements("test.rb", "class Foo < Bar\nend\n") else {
            return;
        };
        assert_eq!(results, vec![("Foo".into(), vec!["Bar".into()])]);
    }

    #[test]
    fn test_implements_dart() {
        let Some(results) =
            extract_implements("test.dart", "class Foo extends Bar implements Baz {}\n")
        else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_d() {
        let Some(results) = extract_implements("test.d", "class Derived : Base, IFoo {}\n") else {
            return;
        };
        assert_eq!(
            results,
            vec![("Derived".into(), vec!["Base".into(), "IFoo".into()])]
        );
    }

    #[test]
    fn test_implements_csharp() {
        let Some(results) = extract_implements("test.cs", "class Foo : Bar, IBaz {}\n") else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "IBaz".into()])]
        );
    }

    #[test]
    fn test_implements_kotlin() {
        let Some(results) = extract_implements("test.kt", "class Foo : Bar(), IBaz {}\n") else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "IBaz".into()])]
        );
    }

    #[test]
    fn test_implements_swift() {
        let Some(results) = extract_implements("test.swift", "class Foo: Bar, Proto {}\n") else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Proto".into()])]
        );
    }

    #[test]
    fn test_implements_php() {
        let Some(results) = extract_implements(
            "test.php",
            "<?php\nclass Foo extends Bar implements Baz {}\n",
        ) else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_objc() {
        let Some(results) = extract_implements("test.mm", "@interface Foo : Bar <Proto>\n@end\n")
        else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Proto".into()])]
        );
    }

    #[test]
    fn test_implements_matlab() {
        // MATLAB and ObjC both use .m; use .m and detect which language we get
        let Some(results) = extract_implements("test.m", "classdef Foo < Bar & Baz\nend\n") else {
            return;
        };
        // If .m resolves to ObjC, this file won't parse as valid ObjC so we get []
        // Skip this test if the extension resolves to the wrong language
        if results.is_empty() {
            // .m resolved to ObjC instead of MATLAB — skip
            return;
        }
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_graphql() {
        let Some(results) = extract_implements(
            "test.graphql",
            "type Foo implements Bar & Baz { id: ID! }\n",
        ) else {
            return;
        };
        assert_eq!(
            results,
            vec![("Foo".into(), vec!["Bar".into(), "Baz".into()])]
        );
    }

    #[test]
    fn test_implements_haskell() {
        let Some(results) =
            extract_implements("test.hs", "instance MyClass Foo where\n  doStuff f = y f\n")
        else {
            return;
        };
        assert_eq!(results, vec![("MyClass".into(), vec!["MyClass".into()])]);
    }

    #[test]
    fn test_go_extract() {
        if parsers::parse_with_grammar("go", "package x").is_none() {
            return; // Go grammar not built; run `cargo xtask build-grammars`.
        }
        let extractor = Extractor::new();
        let content = "package main\n\nfunc helper() {}\n\ntype MyStruct struct {\n    Field int\n}\n\nfunc (m *MyStruct) Method() {}\n\ntype MyInterface interface {\n    Required()\n}\n";
        let result = extractor.extract(&std::path::PathBuf::from("test.go"), content);
        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"helper"), "Should have function helper");
        assert!(names.contains(&"MyStruct"), "Should have struct MyStruct");
        assert!(
            names.contains(&"MyInterface"),
            "Should have interface MyInterface"
        );
    }
}
