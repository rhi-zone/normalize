//! Scope analysis engine using tree-sitter locals queries.
//!
//! Parses `locals.scm` query files to resolve symbol references to their
//! definitions within a single source file. Uses the tree-sitter locals
//! convention:
//!
//! - `@local.scope` — marks a node that creates a new lexical scope
//! - `@local.definition` / `@local.definition.*` — marks a name-binding site
//! - `@local.reference` — marks an identifier that refers to a bound name
//!
//! ## Handling destructuring patterns: `@local.definition.each` + `@local.binding-leaf`
//!
//! Tree-sitter queries only match direct children (`(A (B))` requires `B` to be
//! a named child of `A`). Arbitrarily nested destructuring (`{ a: { b } }`)
//! can't be expressed in a finite set of fixed-depth query patterns.
//!
//! This engine supports two extension captures that work together:
//!
//! - **`@local.binding-leaf`** — declares which node kinds count as binding
//!   identifiers in this language. The engine collects these kinds from all
//!   matches in the query pass.
//! - **`@local.definition.each`** — captures a container node (e.g. a pattern
//!   or parameter node) and triggers recursive descent, emitting a definition
//!   for every descendant leaf whose kind is in the `@local.binding-leaf` set.
//!
//! Example (`javascript.locals.scm`):
//!
//! ```text
//! ; Declare binding leaf kinds for this language
//! (identifier) @local.binding-leaf
//! (shorthand_property_identifier_pattern) @local.binding-leaf
//!
//! ; Recurse into each parameter child — handles f(x), f({ a: { b } }), f([x, y])
//! (formal_parameters (_) @local.definition.each)
//! ```
//!
//! The engine has no hardcoded knowledge of which node kinds are bindings in any
//! given language — that belongs entirely in the `.scm` file.
//!
//! # Usage
//!
//! ```ignore
//! use normalize_scope::ScopeEngine;
//! use normalize_languages::GrammarLoader;
//!
//! let loader = GrammarLoader::new();
//! let engine = ScopeEngine::new(&loader);
//!
//! let refs = engine.find_references("javascript", source, "myVar");
//! for r in refs {
//!     println!("{}:{} -> def at {:?}", r.location.line, r.location.column, r.definition);
//! }
//! ```

use normalize_languages::GrammarLoader;
use streaming_iterator::StreamingIterator;

/// A location in source code.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct Location {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// A reference to a symbol, with optional resolved definition.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct Reference {
    pub name: String,
    pub location: Location,
    /// Definition this reference resolves to, if resolvable via scope walk.
    pub definition: Option<Location>,
}

/// A symbol definition site.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct Definition {
    pub name: String,
    pub location: Location,
}

/// Scope analysis engine backed by tree-sitter locals queries.
///
/// Requires `locals.scm` query files to be present in the grammar search paths
/// (copied there by `cargo xtask build-grammars`).
pub struct ScopeEngine<'a> {
    loader: &'a GrammarLoader,
}

impl<'a> ScopeEngine<'a> {
    pub fn new(loader: &'a GrammarLoader) -> Self {
        Self { loader }
    }

    /// Returns true if locals.scm is available for this language.
    pub fn has_locals(&self, lang: &str) -> bool {
        self.loader.get_locals(lang).is_some()
    }

    /// Find all definitions of `name` in `source`.
    pub fn find_definitions(&self, lang: &str, source: &str, name: &str) -> Vec<Definition> {
        let Some(analysis) = self.analyze(lang, source) else {
            return Vec::new();
        };
        analysis
            .definitions
            .into_iter()
            .filter(|d| d.name == name)
            .map(|d| Definition {
                name: d.name,
                location: d.location,
            })
            .collect()
    }

    /// Find all references to `name` in `source`, with definition resolution.
    ///
    /// Returns both definition sites and reference sites that resolve to any
    /// definition of `name` in this file.
    pub fn find_references(&self, lang: &str, source: &str, name: &str) -> Vec<Reference> {
        let Some(analysis) = self.analyze(lang, source) else {
            return Vec::new();
        };
        analysis
            .references
            .into_iter()
            .filter(|r| r.name == name)
            .collect()
    }

    /// Get all definitions in `source`.
    pub fn all_definitions(&self, lang: &str, source: &str) -> Vec<Definition> {
        let Some(analysis) = self.analyze(lang, source) else {
            return Vec::new();
        };
        analysis
            .definitions
            .into_iter()
            .map(|d| Definition {
                name: d.name,
                location: d.location,
            })
            .collect()
    }

    /// Analyze a source file: collect all scopes, definitions, and references,
    /// resolving each reference to its definition via scope walk.
    fn analyze(&self, lang: &str, source: &str) -> Option<FileAnalysis> {
        let grammar = self.loader.get(lang)?;
        let locals_src = self.loader.get_locals(lang)?;

        let query = tree_sitter::Query::new(&grammar, &locals_src).ok()?;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).ok()?;
        let tree = parser.parse(source, None)?;

        let capture_names: Vec<String> = query
            .capture_names()
            .iter()
            .map(|s| s.to_string())
            .collect();

        // First pass: collect binding leaf kinds declared via @local.binding-leaf
        // and defer @local.definition.each nodes (can't expand until leaf kinds known).
        let mut binding_leaf_kinds: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut deferred_each: Vec<tree_sitter::Node> = Vec::new();
        let mut scopes: Vec<ScopeRange> = Vec::new();
        let mut raw_defs: Vec<RawCapture> = Vec::new();
        let mut raw_refs: Vec<RawCapture> = Vec::new();

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            // Evaluate custom predicates (general_predicates) that tree-sitter
            // doesn't handle natively. This covers `#is-match-op!` and similar
            // language-specific filters in locals.scm files.
            if !check_general_predicates(m, &query, source.as_bytes()) {
                continue;
            }

            for cap in m.captures {
                let name = &capture_names[cap.index as usize];
                let node = cap.node;
                let Ok(text) = node.utf8_text(source.as_bytes()) else {
                    continue;
                };

                let loc = Location {
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                };

                if name == "local.binding-leaf" {
                    // Declares which leaf node kinds @local.definition.each should
                    // collect when recursing. The .scm file is the authority on
                    // what counts as a binding identifier in that language.
                    binding_leaf_kinds.insert(node.kind().to_string());
                } else if name == "local.definition.each" {
                    // Defer: we need binding_leaf_kinds fully populated first.
                    deferred_each.push(node);
                } else if name == "local.scope" {
                    scopes.push(ScopeRange {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                    });
                } else if name.starts_with("local.definition") {
                    raw_defs.push(RawCapture {
                        name: text.to_string(),
                        location: loc,
                    });
                } else if name == "local.reference" {
                    raw_refs.push(RawCapture {
                        name: text.to_string(),
                        location: loc,
                    });
                }
            }
        }

        // Expand deferred @local.definition.each nodes now that binding_leaf_kinds is complete.
        for node in deferred_each {
            collect_binding_identifiers(
                node,
                source.as_bytes(),
                &binding_leaf_kinds,
                &mut raw_defs,
            );
        }

        // Resolve references to definitions via scope walk
        let references: Vec<Reference> = raw_refs
            .into_iter()
            .map(|r| {
                let definition = resolve_reference(&r, &scopes, &raw_defs);
                Reference {
                    name: r.name,
                    location: r.location,
                    definition,
                }
            })
            .collect();

        Some(FileAnalysis {
            definitions: raw_defs,
            references,
        })
    }
}

// ── Custom predicate evaluation ─────────────────────────────────────────────

/// Evaluate custom (general) predicates that tree-sitter doesn't handle natively.
///
/// Built-in text predicates (`#eq?`, `#any-of?`, `#match?`) are evaluated
/// automatically by `QueryCursor::matches` via `satisfies_text_predicates`.
/// However, they only work reliably on **named** node captures. Unnamed node
/// captures in field position (e.g. `operator: _ @op`) are silently skipped.
///
/// This function handles custom predicates defined in locals.scm files:
///
/// - `#is-match-op!` — checks that the captured node has an unnamed `=` child.
///   Used to filter `binary_operator` matches to only assignment (`x = expr`).
///
/// Unknown predicates pass through (return true) to avoid breaking other queries.
fn check_general_predicates(
    m: &tree_sitter::QueryMatch<'_, '_>,
    query: &tree_sitter::Query,
    source: &[u8],
) -> bool {
    use tree_sitter::QueryPredicateArg;
    query
        .general_predicates(m.pattern_index)
        .iter()
        .all(|pred| match pred.operator.as_ref() {
            "is-match-op!" => {
                // Expect one capture arg: the node to inspect for an `=` child.
                if let Some(QueryPredicateArg::Capture(cap_idx)) = pred.args.first() {
                    m.captures
                        .iter()
                        .filter(|c| c.index == *cap_idx)
                        .any(|c| node_has_unnamed_child(c.node, "=", source))
                } else {
                    true
                }
            }
            _ => true,
        })
}

/// Returns true if `node` has an unnamed child whose source text equals `text`.
fn node_has_unnamed_child(node: tree_sitter::Node<'_>, text: &str, source: &[u8]) -> bool {
    (0..node.child_count()).any(|i| {
        node.child(i as u32)
            .is_some_and(|child| !child.is_named() && child.utf8_text(source) == Ok(text))
    })
}

// ── Internal types ──────────────────────────────────────────────────────────

struct FileAnalysis {
    definitions: Vec<RawCapture>,
    references: Vec<Reference>,
}

struct ScopeRange {
    start_byte: usize,
    end_byte: usize,
}

struct RawCapture {
    name: String,
    location: Location,
}

/// Resolve a reference to its definition by walking up the scope chain.
///
/// Algorithm:
/// 1. Find all scope ranges that contain the reference (by byte offset)
/// 2. Sort them innermost-first (smallest range = most specific scope)
/// 3. For each scope: look for a definition with matching name that:
///    a. Is within that scope's byte range
///    b. Appears before the reference (textual order, handles forward refs only for types)
/// 4. Return the first match found
fn resolve_reference(
    r: &RawCapture,
    scopes: &[ScopeRange],
    definitions: &[RawCapture],
) -> Option<Location> {
    let ref_start = r.location.start_byte;

    // Find all scopes containing this reference, sorted innermost-first
    let mut containing: Vec<&ScopeRange> = scopes
        .iter()
        .filter(|s| s.start_byte <= ref_start && ref_start < s.end_byte)
        .collect();
    // Sort by scope size ascending (smallest = innermost = highest priority)
    containing.sort_by_key(|s| s.end_byte - s.start_byte);

    for scope in &containing {
        let def = definitions.iter().find(|d| {
            d.name == r.name
                && d.location.start_byte >= scope.start_byte
                && d.location.start_byte < scope.end_byte
                && d.location.start_byte < ref_start
        });
        if let Some(d) = def {
            return Some(d.location.clone());
        }
    }

    None
}

/// Recursively collect all binding identifier leaf nodes from a pattern node.
///
/// Used by `@local.definition.each`. Recurses into the subtree and emits a
/// `RawCapture` for every named leaf node whose kind is in `binding_leaf_kinds`.
/// That set is populated from `@local.binding-leaf` captures in the same query,
/// so the `.scm` file (not the engine) defines what counts as a binding identifier.
fn collect_binding_identifiers(
    node: tree_sitter::Node,
    source: &[u8],
    binding_leaf_kinds: &std::collections::HashSet<String>,
    out: &mut Vec<RawCapture>,
) {
    if !node.is_named() {
        return;
    }
    if node.child_count() == 0 {
        if binding_leaf_kinds.contains(node.kind()) {
            if let Ok(text) = node.utf8_text(source) {
                out.push(RawCapture {
                    name: text.to_string(),
                    location: Location {
                        line: node.start_position().row + 1,
                        column: node.start_position().column + 1,
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                    },
                });
            }
        }
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_binding_identifiers(child, source, binding_leaf_kinds, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar_dir() -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("target/grammars"));
        p.filter(|p| p.exists())
    }

    fn loader() -> GrammarLoader {
        let mut l = GrammarLoader::new();
        if let Some(dir) = grammar_dir() {
            l.add_path(dir);
        }
        l
    }

    #[test]
    fn test_has_locals_javascript() {
        let l = loader();
        let engine = ScopeEngine::new(&l);
        // If grammars aren't built, skip gracefully
        if l.get("javascript").is_none() {
            return;
        }
        // javascript has locals.scm in arborium
        let _ = engine.has_locals("javascript");
    }

    #[test]
    fn test_no_locals_graceful() {
        let l = GrammarLoader::with_paths(vec![]);
        let engine = ScopeEngine::new(&l);
        let refs = engine.find_references("rust", "fn main() {}", "main");
        assert!(
            refs.is_empty(),
            "should return empty when no grammar/locals"
        );
    }

    fn skip_if_no(l: &GrammarLoader, lang: &str) -> bool {
        l.get(lang).is_none() || l.get_locals(lang).is_none()
    }

    fn skip_if_no_rust(l: &GrammarLoader) -> bool {
        skip_if_no(l, "rust")
    }

    #[test]
    fn test_rust_has_locals() {
        let l = loader();
        if l.get("rust").is_none() {
            return;
        }
        let engine = ScopeEngine::new(&l);
        assert!(
            engine.has_locals("rust"),
            "rust.locals.scm should be present after xtask build-grammars"
        );
    }

    #[test]
    fn test_rust_function_parameter() {
        let l = loader();
        if skip_if_no_rust(&l) {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn add(x: i32, y: i32) -> i32 { x + y }";
        // `x` should be found: one definition (parameter) and one reference (body)
        let refs = engine.find_references("rust", src, "x");
        assert!(!refs.is_empty(), "x should appear as reference");
        let has_def = refs.iter().any(|r| r.definition.is_some());
        assert!(
            has_def,
            "x reference should resolve to its parameter definition"
        );
        let defs = engine.find_definitions("rust", src, "x");
        assert_eq!(defs.len(), 1, "x should have exactly one definition");
    }

    #[test]
    fn test_rust_let_binding() {
        let l = loader();
        if skip_if_no_rust(&l) {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn f() { let v = 1; let w = v + 1; w }";
        let defs = engine.find_definitions("rust", src, "v");
        assert_eq!(defs.len(), 1, "v should have one definition");
        let refs = engine.find_references("rust", src, "v");
        // At least one reference to v that resolves to the let binding
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "v reference in body should resolve to let binding"
        );
    }

    #[test]
    fn test_rust_for_loop_variable() {
        let l = loader();
        if skip_if_no_rust(&l) {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn f() { for i in 0..10 { let _ = i; } }";
        let defs = engine.find_definitions("rust", src, "i");
        assert_eq!(
            defs.len(),
            1,
            "for loop variable i should have one definition"
        );
        let refs = engine.find_references("rust", src, "i");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "i inside loop should resolve to for pattern");
    }

    #[test]
    fn test_rust_closure_parameter() {
        let l = loader();
        if skip_if_no_rust(&l) {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn f() { let g = |a: i32| a * 2; }";
        let defs = engine.find_definitions("rust", src, "a");
        assert_eq!(defs.len(), 1, "closure param a should have one definition");
        let refs = engine.find_references("rust", src, "a");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "a in closure body should resolve to closure param"
        );
    }

    #[test]
    fn test_rust_no_cross_scope_leakage() {
        let l = loader();
        if skip_if_no_rust(&l) {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // x in first function should not resolve to x in second function
        let src = "fn f(x: i32) -> i32 { x } fn g(x: i32) -> i32 { x }";
        let defs = engine.find_definitions("rust", src, "x");
        assert_eq!(defs.len(), 2, "two separate x parameter definitions");
    }

    // ── Python ───────────────────────────────────────────────────────────────

    #[test]
    fn test_python_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "python") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def add(x, y):\n    return x + y\n";
        let defs = engine.find_definitions("python", src, "x");
        assert_eq!(defs.len(), 1, "python: x should have one definition");
        let refs = engine.find_references("python", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "python: x reference should resolve to param");
    }

    #[test]
    fn test_python_assignment() {
        let l = loader();
        if skip_if_no(&l, "python") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f():\n    v = 1\n    return v\n";
        let defs = engine.find_definitions("python", src, "v");
        assert_eq!(defs.len(), 1, "python: v should have one definition");
    }

    #[test]
    fn test_python_for_variable() {
        let l = loader();
        if skip_if_no(&l, "python") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f():\n    for i in range(10):\n        print(i)\n";
        let defs = engine.find_definitions("python", src, "i");
        assert_eq!(defs.len(), 1, "python: for loop variable i");
    }

    // ── Go ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_go_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "go") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "package p\nfunc add(x int, y int) int { return x + y }\n";
        let defs = engine.find_definitions("go", src, "x");
        assert_eq!(defs.len(), 1, "go: x should have one definition");
        let refs = engine.find_references("go", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "go: x reference should resolve to param");
    }

    #[test]
    fn test_go_short_var_decl() {
        let l = loader();
        if skip_if_no(&l, "go") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "package p\nfunc f() {\n    v := 1\n    _ = v\n}\n";
        let defs = engine.find_definitions("go", src, "v");
        assert_eq!(defs.len(), 1, "go: short var decl v");
    }

    // ── Java ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_java_method_parameter() {
        let l = loader();
        if skip_if_no(&l, "java") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "class A { int add(int x, int y) { return x + y; } }\n";
        let defs = engine.find_definitions("java", src, "x");
        assert_eq!(defs.len(), 1, "java: x should have one definition");
        let refs = engine.find_references("java", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "java: x should resolve to param def");
    }

    #[test]
    fn test_java_local_variable() {
        let l = loader();
        if skip_if_no(&l, "java") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "class A { void f() { int v = 1; int w = v + 1; } }\n";
        let defs = engine.find_definitions("java", src, "v");
        assert_eq!(defs.len(), 1, "java: local variable v");
    }

    // ── C ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_c_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "c") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "int add(int x, int y) { return x + y; }\n";
        let defs = engine.find_definitions("c", src, "x");
        assert_eq!(defs.len(), 1, "c: x should have one definition");
        let refs = engine.find_references("c", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "c: x reference should resolve to param");
    }

    #[test]
    fn test_c_local_variable() {
        let l = loader();
        if skip_if_no(&l, "c") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "void f() { int v = 1; int w = v + 1; }\n";
        let defs = engine.find_definitions("c", src, "v");
        assert_eq!(defs.len(), 1, "c: local variable v");
    }

    // ── C++ ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_cpp_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "cpp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "int add(int x, int y) { return x + y; }\n";
        let defs = engine.find_definitions("cpp", src, "x");
        assert_eq!(defs.len(), 1, "cpp: x should have one definition");
        let refs = engine.find_references("cpp", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "cpp: x reference should resolve to param");
    }

    #[test]
    fn test_cpp_local_variable() {
        let l = loader();
        if skip_if_no(&l, "cpp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "void f() { int v = 1; int w = v + 1; }\n";
        let defs = engine.find_definitions("cpp", src, "v");
        assert_eq!(defs.len(), 1, "cpp: local variable v");
    }

    // ── C# ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_csharp_method_parameter() {
        let l = loader();
        if skip_if_no(&l, "c-sharp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "class A { int Add(int x, int y) { return x + y; } }\n";
        let defs = engine.find_definitions("c-sharp", src, "x");
        assert_eq!(defs.len(), 1, "c#: x should have one definition");
        let refs = engine.find_references("c-sharp", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "c#: x reference should resolve to param");
    }

    #[test]
    fn test_csharp_local_variable() {
        let l = loader();
        if skip_if_no(&l, "c-sharp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "class A { void F() { int v = 1; int w = v + 1; } }\n";
        let defs = engine.find_definitions("c-sharp", src, "v");
        assert_eq!(defs.len(), 1, "c#: local variable v");
    }

    // ── Ruby ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_ruby_method_parameter() {
        let l = loader();
        if skip_if_no(&l, "ruby") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def add(x, y)\n  x + y\nend\n";
        let defs = engine.find_definitions("ruby", src, "x");
        assert_eq!(defs.len(), 1, "ruby: x should have one definition");
        let refs = engine.find_references("ruby", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "ruby: x reference should resolve to param");
    }

    #[test]
    fn test_ruby_assignment() {
        let l = loader();
        if skip_if_no(&l, "ruby") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f\n  v = 1\n  v + 2\nend\n";
        let defs = engine.find_definitions("ruby", src, "v");
        assert_eq!(defs.len(), 1, "ruby: assignment v");
    }

    // ── Bash ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_bash_function_and_variable() {
        let l = loader();
        if skip_if_no(&l, "bash") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "greet() {\n  name=\"world\"\n  echo \"$name\"\n}\n";
        let defs = engine.find_definitions("bash", src, "name");
        assert_eq!(defs.len(), 1, "bash: variable assignment name");
    }

    #[test]
    fn test_bash_for_variable() {
        let l = loader();
        if skip_if_no(&l, "bash") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "for item in a b c; do\n  echo \"$item\"\ndone\n";
        let defs = engine.find_definitions("bash", src, "item");
        assert_eq!(defs.len(), 1, "bash: for loop variable item");
    }

    // ── Kotlin ────────────────────────────────────────────────────────────────

    #[test]
    fn test_kotlin_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "kotlin") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fun add(x: Int, y: Int): Int = x + y\n";
        let defs = engine.find_definitions("kotlin", src, "x");
        assert_eq!(defs.len(), 1, "kotlin: x should have one definition");
        let resolved = engine.find_references("kotlin", src, "x");
        assert!(!resolved.is_empty(), "kotlin: x reference should exist");
    }

    #[test]
    fn test_kotlin_variable_declaration() {
        let l = loader();
        if skip_if_no(&l, "kotlin") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fun f() {\n    val v = 1\n    val w = v + 1\n}\n";
        let defs = engine.find_definitions("kotlin", src, "v");
        assert_eq!(defs.len(), 1, "kotlin: val declaration v");
    }

    // ── PHP ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_php_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "php") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "<?php\nfunction add($x, $y) {\n    return $x + $y;\n}\n";
        // PHP variables are captured as variable_name including $ sigil
        let defs = engine.find_definitions("php", src, "$x");
        assert_eq!(defs.len(), 1, "php: $x should have one definition");
        let refs = engine.find_references("php", src, "$x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "php: $x reference should resolve to param");
    }

    // ── Zig ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_zig_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "zig") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn add(x: i32, y: i32) i32 { return x + y; }\n";
        // ParamDecl.parameter captures x as a definition
        let defs = engine.find_definitions("zig", src, "x");
        assert_eq!(defs.len(), 1, "zig: x should have one definition");
        // References are captured (IDENTIFIER matches everywhere including body)
        let refs = engine.find_references("zig", src, "x");
        assert!(!refs.is_empty(), "zig: x should appear as a reference");
    }

    #[test]
    fn test_zig_var_decl() {
        let l = loader();
        if skip_if_no(&l, "zig") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn f() void {\n    const v = 1;\n    const w = v + 1;\n}\n";
        let defs = engine.find_definitions("zig", src, "v");
        assert_eq!(defs.len(), 1, "zig: const declaration v");
    }

    // ── Dart ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_dart_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "dart") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "int add(int x, int y) { return x + y; }\n";
        // Parameters are captured as definitions (within function_signature scope).
        // Cross-scope resolution to the sibling function_body is not supported by
        // this grammar structure, so we only verify the definition is found.
        let defs = engine.find_definitions("dart", src, "x");
        assert_eq!(defs.len(), 1, "dart: x should have one definition");
    }

    #[test]
    fn test_dart_local_variable() {
        let l = loader();
        if skip_if_no(&l, "dart") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "void f() {\n  var result = 42;\n  print(result);\n}\n";
        let defs = engine.find_definitions("dart", src, "result");
        assert_eq!(defs.len(), 1, "dart: local variable result");
        let refs = engine.find_references("dart", src, "result");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "dart: result reference should resolve");
    }

    // ── Elixir ────────────────────────────────────────────────────────────────

    #[test]
    fn test_elixir_anon_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "elixir") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "f = fn x, y -> x + y end\n";
        let defs = engine.find_definitions("elixir", src, "x");
        assert_eq!(defs.len(), 1, "elixir: anonymous function parameter x");
        let refs = engine.find_references("elixir", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "elixir: x reference should resolve to param");
    }

    #[test]
    fn test_elixir_pattern_match() {
        let l = loader();
        if skip_if_no(&l, "elixir") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // x = 42 defines x via the "=" string literal pattern
        let src = "x = 42\n";
        let defs = engine.find_definitions("elixir", src, "x");
        assert_eq!(defs.len(), 1, "elixir: x = 42 should define x");
    }

    #[test]
    fn test_elixir_no_false_definitions() {
        let l = loader();
        if skip_if_no(&l, "elixir") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // x + y should not produce definitions — "=" literal pattern only matches =
        let src = "x + y\n";
        let defs = engine.find_definitions("elixir", src, "x");
        assert_eq!(
            defs.len(),
            0,
            "elixir: x in x + y should not be a definition"
        );
    }

    // ── Erlang ────────────────────────────────────────────────────────────────

    #[test]
    fn test_erlang_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "erlang") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "add(X, Y) -> X + Y.\n";
        let defs = engine.find_definitions("erlang", src, "X");
        assert_eq!(defs.len(), 1, "erlang: function parameter X");
        let refs = engine.find_references("erlang", src, "X");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "erlang: X reference should resolve to param");
    }

    #[test]
    fn test_erlang_anon_function() {
        let l = loader();
        if skip_if_no(&l, "erlang") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "F = fun(X) -> X * 2 end.\n";
        let defs = engine.find_definitions("erlang", src, "X");
        assert_eq!(defs.len(), 1, "erlang: anonymous function parameter X");
    }

    // ── Clojure ───────────────────────────────────────────────────────────────

    #[test]
    fn test_clojure_defn_name() {
        let l = loader();
        if skip_if_no(&l, "clojure") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "(defn add [x y] (+ x y))\n";
        let defs = engine.find_definitions("clojure", src, "add");
        assert_eq!(defs.len(), 1, "clojure: defn should define function name");
    }

    #[test]
    fn test_clojure_fn_parameter() {
        let l = loader();
        if skip_if_no(&l, "clojure") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "(fn [x y] (+ x y))\n";
        let defs = engine.find_definitions("clojure", src, "x");
        assert_eq!(defs.len(), 1, "clojure: fn parameter x");
        let refs = engine.find_references("clojure", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "clojure: x reference should resolve to param"
        );
    }

    #[test]
    fn test_clojure_let_binding() {
        let l = loader();
        if skip_if_no(&l, "clojure") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "(let [x 1] (inc x))\n";
        let defs = engine.find_definitions("clojure", src, "x");
        assert_eq!(defs.len(), 1, "clojure: let binding x");
        let refs = engine.find_references("clojure", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "clojure: x in (inc x) should resolve");
    }

    #[test]
    fn test_clojure_no_false_scope() {
        let l = loader();
        if skip_if_no(&l, "clojure") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // A regular function call (not a def form) should not create definitions
        let src = "(foo x y)\n";
        let defs = engine.find_definitions("clojure", src, "foo");
        assert_eq!(defs.len(), 0, "clojure: (foo x y) should not define foo");
    }

    // ── Julia ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_julia_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "julia") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function add(x, y)\n  x + y\nend\n";
        let defs = engine.find_definitions("julia", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "julia: function param x should have one definition"
        );
        let refs = engine.find_references("julia", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "julia: x reference should resolve to param");
    }

    #[test]
    fn test_julia_for_variable() {
        let l = loader();
        if skip_if_no(&l, "julia") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "for i in 1:10\n  println(i)\nend\n";
        let defs = engine.find_definitions("julia", src, "i");
        assert_eq!(defs.len(), 1, "julia: for loop variable i");
        let refs = engine.find_references("julia", src, "i");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "julia: i reference should resolve to for binding"
        );
    }

    #[test]
    fn test_julia_let_binding() {
        let l = loader();
        if skip_if_no(&l, "julia") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let a = 1\n  a + 1\nend\n";
        let defs = engine.find_definitions("julia", src, "a");
        assert_eq!(defs.len(), 1, "julia: let binding a");
    }

    // ── Perl ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_perl_my_scalar() {
        let l = loader();
        if skip_if_no(&l, "perl") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // my $name defines "name"; print $name references "name"
        let src = "sub greet {\n  my $name = \"world\";\n  print $name;\n}\n";
        let defs = engine.find_definitions("perl", src, "name");
        assert_eq!(defs.len(), 1, "perl: my $name should define name");
        let refs = engine.find_references("perl", src, "name");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "perl: $name reference should resolve");
    }

    #[test]
    fn test_perl_my_list() {
        let l = loader();
        if skip_if_no(&l, "perl") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "my ($a, $b) = (1, 2);\n";
        let defs_a = engine.find_definitions("perl", src, "a");
        assert_eq!(defs_a.len(), 1, "perl: my ($a, $b) should define a");
        let defs_b = engine.find_definitions("perl", src, "b");
        assert_eq!(defs_b.len(), 1, "perl: my ($a, $b) should define b");
    }

    // ── Groovy ────────────────────────────────────────────────────────────────

    #[test]
    fn test_groovy_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "groovy") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def add(x, y) {\n  return x + y\n}\n";
        let defs = engine.find_definitions("groovy", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "groovy: function param x should have one definition"
        );
        let refs = engine.find_references("groovy", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "groovy: x reference should resolve to param");
    }

    #[test]
    fn test_groovy_closure_parameter() {
        let l = loader();
        if skip_if_no(&l, "groovy") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f = { a, b -> a + b }\n";
        let defs = engine.find_definitions("groovy", src, "a");
        assert_eq!(
            defs.len(),
            1,
            "groovy: closure param a should have one definition"
        );
        let refs = engine.find_references("groovy", src, "a");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "groovy: a reference should resolve to closure param"
        );
    }

    #[test]
    fn test_groovy_variable_declaration() {
        let l = loader();
        if skip_if_no(&l, "groovy") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f() {\n  def v = 1\n  return v\n}\n";
        let defs = engine.find_definitions("groovy", src, "v");
        assert_eq!(defs.len(), 1, "groovy: def v should define v");
    }

    // ── D ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_d_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "d") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "int add(int x, int y) {\n  return x + y;\n}\n";
        let defs = engine.find_definitions("d", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "d: function param x should have one definition"
        );
        let refs = engine.find_references("d", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "d: x reference should resolve to param");
    }

    #[test]
    fn test_d_auto_declaration() {
        let l = loader();
        if skip_if_no(&l, "d") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "void f() {\n  auto v = 42;\n  writeln(v);\n}\n";
        let defs = engine.find_definitions("d", src, "v");
        assert_eq!(defs.len(), 1, "d: auto v should define v");
        let refs = engine.find_references("d", src, "v");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "d: v reference should resolve to auto binding"
        );
    }

    // ── TypeScript ────────────────────────────────────────────────────────────

    #[test]
    fn test_typescript_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function add(x: number, y: number): number { return x + y; }";
        let defs = engine.find_definitions("typescript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "typescript: required parameter x should have one definition"
        );
        let refs = engine.find_references("typescript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "typescript: x reference should resolve to param"
        );
    }

    #[test]
    fn test_typescript_variable_declarator() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // const inside a function scope so resolution works
        let src = "function f() { const x = 1; return x; }";
        let defs = engine.find_definitions("typescript", src, "x");
        assert_eq!(defs.len(), 1, "typescript: const x should define x");
        let refs = engine.find_references("typescript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "typescript: x reference should resolve to const"
        );
    }

    #[test]
    fn test_typescript_function_declaration() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function greet(name: string): void { console.log(name); }";
        let defs = engine.find_definitions("typescript", src, "greet");
        assert_eq!(
            defs.len(),
            1,
            "typescript: function declaration greet should be defined"
        );
    }

    #[test]
    fn test_typescript_arrow_function_single_param() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "const double = x => x * 2;";
        let defs = engine.find_definitions("typescript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "typescript: arrow function single param x should be defined"
        );
        let refs = engine.find_references("typescript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "typescript: x in arrow body should resolve");
    }

    #[test]
    fn test_typescript_object_destructuring_param() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f({ a, b }: T) { return a + b; }";
        let defs_a = engine.find_definitions("typescript", src, "a");
        assert_eq!(
            defs_a.len(),
            1,
            "typescript: destructured param a should be defined"
        );
        let refs_a = engine.find_references("typescript", src, "a");
        let resolved = refs_a.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "typescript: a in body should resolve to destructured param"
        );
    }

    #[test]
    fn test_typescript_array_destructuring_param() {
        let l = loader();
        if skip_if_no(&l, "typescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f([x, y]: U) { return x + y; }";
        let defs = engine.find_definitions("typescript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "typescript: destructured array param x should be defined"
        );
        let refs = engine.find_references("typescript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "typescript: x in body should resolve to destructured param"
        );
    }

    // ── JavaScript ────────────────────────────────────────────────────────────

    #[test]
    fn test_javascript_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function add(x, y) { return x + y; }";
        let defs = engine.find_definitions("javascript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "javascript: function param x should have one definition"
        );
        let refs = engine.find_references("javascript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "javascript: x reference should resolve to param"
        );
    }

    #[test]
    fn test_javascript_variable_declarator() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // const inside a function scope so resolution works
        let src = "function f() { const x = 1; return x; }";
        let defs = engine.find_definitions("javascript", src, "x");
        assert_eq!(defs.len(), 1, "javascript: const x should define x");
        let refs = engine.find_references("javascript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "javascript: x reference should resolve");
    }

    #[test]
    fn test_javascript_function_name() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function greet() { return 'hello'; }";
        let defs = engine.find_definitions("javascript", src, "greet");
        assert_eq!(
            defs.len(),
            1,
            "javascript: function declaration greet should be defined"
        );
    }

    #[test]
    fn test_javascript_arrow_function_single_param() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "const double = x => x * 2;";
        let defs = engine.find_definitions("javascript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "javascript: arrow single param x should be defined"
        );
        let refs = engine.find_references("javascript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "javascript: x in arrow body should resolve");
    }

    #[test]
    fn test_javascript_object_destructuring_param() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f({ a, b }) { return a + b; }";
        let defs_a = engine.find_definitions("javascript", src, "a");
        assert_eq!(
            defs_a.len(),
            1,
            "javascript: destructured param a should be defined"
        );
        let refs_a = engine.find_references("javascript", src, "a");
        let resolved = refs_a.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "javascript: a in body should resolve to destructured param"
        );
    }

    #[test]
    fn test_javascript_array_destructuring_param() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f([x, y]) { return x + y; }";
        let defs = engine.find_definitions("javascript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "javascript: destructured array param x should be defined"
        );
        let refs = engine.find_references("javascript", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "javascript: x in body should resolve to destructured param"
        );
    }

    #[test]
    fn test_javascript_default_param() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f(c = 1) { return c; }";
        let defs = engine.find_definitions("javascript", src, "c");
        assert_eq!(
            defs.len(),
            1,
            "javascript: default param c should be defined"
        );
        let refs = engine.find_references("javascript", src, "c");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "javascript: c in body should resolve to default param"
        );
    }

    #[test]
    fn test_javascript_nested_destructuring_param() {
        let l = loader();
        if skip_if_no(&l, "javascript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // Nested: { a: { b } } — b is two levels deep inside object_pattern
        let src = "function f({ a: { b } }) { return b; }";
        let defs = engine.find_definitions("javascript", src, "b");
        assert_eq!(
            defs.len(),
            1,
            "javascript: nested destructured param b should be defined"
        );
        let refs = engine.find_references("javascript", src, "b");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(
            resolved >= 1,
            "javascript: b should resolve from nested destructuring"
        );
    }

    // ── Lua ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_lua_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "lua") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function add(x, y) return x + y end";
        let defs = engine.find_definitions("lua", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "lua: function param x should have one definition"
        );
        let refs = engine.find_references("lua", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "lua: x reference should resolve to param");
    }

    #[test]
    fn test_lua_function_name() {
        let l = loader();
        if skip_if_no(&l, "lua") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function greet() return 'hello' end";
        let defs = engine.find_definitions("lua", src, "greet");
        assert_eq!(
            defs.len(),
            1,
            "lua: function declaration greet should be defined"
        );
    }

    #[test]
    fn test_lua_for_numeric() {
        let l = loader();
        if skip_if_no(&l, "lua") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "for i = 1, 10 do print(i) end";
        let defs = engine.find_definitions("lua", src, "i");
        assert_eq!(
            defs.len(),
            1,
            "lua: for numeric variable i should be defined"
        );
        let refs = engine.find_references("lua", src, "i");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "lua: i in for body should resolve");
    }

    // ── Scala ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_scala_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "scala") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def add(x: Int, y: Int): Int = x + y";
        let defs = engine.find_definitions("scala", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "scala: function param x should have one definition"
        );
        let refs = engine.find_references("scala", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "scala: x reference should resolve to param");
    }

    #[test]
    fn test_scala_val_definition() {
        let l = loader();
        if skip_if_no(&l, "scala") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "val x = 42";
        let defs = engine.find_definitions("scala", src, "x");
        assert_eq!(defs.len(), 1, "scala: val x should be defined");
    }

    #[test]
    fn test_scala_function_name() {
        let l = loader();
        if skip_if_no(&l, "scala") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def greet(name: String): String = \"hello \" + name";
        let defs = engine.find_definitions("scala", src, "greet");
        assert_eq!(
            defs.len(),
            1,
            "scala: def greet should define function name"
        );
    }

    // ── R ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_r_arrow_assignment() {
        let l = loader();
        if skip_if_no(&l, "r") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // assignment inside function scope so x resolves
        let src = "f <- function(a) { x <- a * 2; x }\n";
        let defs = engine.find_definitions("r", src, "x");
        assert_eq!(defs.len(), 1, "r: x <- ... should define x");
        let refs = engine.find_references("r", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "r: x reference in body should resolve");
    }

    #[test]
    fn test_r_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "r") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "f <- function(a, b) { a + b }\n";
        let defs = engine.find_definitions("r", src, "a");
        assert_eq!(defs.len(), 1, "r: function param a should be defined");
        let refs = engine.find_references("r", src, "a");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "r: a reference in body should resolve");
    }

    // ── OCaml ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_ocaml_let_binding() {
        let l = loader();
        if skip_if_no(&l, "ocaml") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // Let binding: x is a value_pattern (definition)
        let src = "let x = 42 in x + 1";
        let defs = engine.find_definitions("ocaml", src, "x");
        assert_eq!(defs.len(), 1, "ocaml: let x = 42 should define x");
    }

    #[test]
    fn test_ocaml_function_params() {
        let l = loader();
        if skip_if_no(&l, "ocaml") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // OCaml curried function: params are value_patterns
        let src = "let add x y = x + y";
        let defs = engine.find_definitions("ocaml", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "ocaml: curried function param x should be defined"
        );
    }

    // ── TSX ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_tsx_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "tsx") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function add(x: number, y: number): number { return x + y; }";
        let defs = engine.find_definitions("tsx", src, "x");
        assert_eq!(defs.len(), 1, "tsx: required parameter x should be defined");
        let refs = engine.find_references("tsx", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "tsx: x reference should resolve to param");
    }

    #[test]
    fn test_tsx_variable_declarator() {
        let l = loader();
        if skip_if_no(&l, "tsx") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "function f() { const x = 1; return x; }";
        let defs = engine.find_definitions("tsx", src, "x");
        assert_eq!(defs.len(), 1, "tsx: const x should define x");
    }

    // ── Gleam ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_gleam_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "gleam") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn add(x, y) { x + y }";
        let defs = engine.find_definitions("gleam", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "gleam: function_parameter x should be defined"
        );
        let refs = engine.find_references("gleam", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "gleam: x reference should resolve to param");
    }

    #[test]
    fn test_gleam_let_binding() {
        let l = loader();
        if skip_if_no(&l, "gleam") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "fn f() { let x = 1 x }";
        let defs = engine.find_definitions("gleam", src, "x");
        assert_eq!(defs.len(), 1, "gleam: let x should define x");
    }

    // ── TLA+ ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_tlaplus_operator_definition() {
        let l = loader();
        if skip_if_no(&l, "tlaplus") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // TLA+ operator definition with parameters
        let src = "---- MODULE Test ----\nOp(x, y) == x + y\n====";
        let defs = engine.find_definitions("tlaplus", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "tlaplus: operator parameter x should be defined"
        );
        let refs = engine.find_references("tlaplus", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "tlaplus: x reference should resolve");
    }

    #[test]
    fn test_tlaplus_let_in() {
        let l = loader();
        if skip_if_no(&l, "tlaplus") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "---- MODULE Test ----\nExpr == LET x == 1 IN x + 1\n====";
        let defs = engine.find_definitions("tlaplus", src, "x");
        assert_eq!(defs.len(), 1, "tlaplus: LET x should define x");
    }

    // ── Swift ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_swift_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "swift") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // Swift function with internal parameter name
        let src = "func add(_ x: Int, _ y: Int) -> Int { return x + y }";
        let defs = engine.find_definitions("swift", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "swift: function parameter x should be defined"
        );
        let refs = engine.find_references("swift", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "swift: x reference should resolve to param");
    }

    #[test]
    fn test_swift_function_name() {
        let l = loader();
        if skip_if_no(&l, "swift") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "func greet() -> String { return \"hello\" }";
        let defs = engine.find_definitions("swift", src, "greet");
        assert_eq!(
            defs.len(),
            1,
            "swift: function name greet should be defined"
        );
    }

    // ── Elm ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_elm_function_definition() {
        let l = loader();
        if skip_if_no(&l, "elm") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // Elm function with parameters
        let src = "add x y = x + y";
        let defs = engine.find_definitions("elm", src, "x");
        assert_eq!(defs.len(), 1, "elm: function parameter x should be defined");
    }

    // ── F# ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_fsharp_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "fsharp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let add x y = x + y";
        let defs = engine.find_definitions("fsharp", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "fsharp: function parameter x should be defined"
        );
    }

    #[test]
    fn test_fsharp_value_binding() {
        let l = loader();
        if skip_if_no(&l, "fsharp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let x = 42";
        let defs = engine.find_definitions("fsharp", src, "x");
        assert_eq!(defs.len(), 1, "fsharp: let x should define x");
    }

    // ── Ada ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_ada_parameter() {
        let l = loader();
        if skip_if_no(&l, "ada") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "procedure Add(X : Integer; Y : Integer) is begin null; end Add;";
        let defs = engine.find_definitions("ada", src, "X");
        assert_eq!(defs.len(), 1, "ada: parameter X should be defined");
    }

    // ── Starlark ──────────────────────────────────────────────────────────────

    #[test]
    fn test_starlark_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "starlark") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def add(x, y):\n  return x + y\n";
        let defs = engine.find_definitions("starlark", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "starlark: function param x should be defined"
        );
        let refs = engine.find_references("starlark", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "starlark: x in body should resolve");
    }

    #[test]
    fn test_starlark_assignment() {
        let l = loader();
        if skip_if_no(&l, "starlark") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "def f():\n  x = 1\n  return x\n";
        let defs = engine.find_definitions("starlark", src, "x");
        assert_eq!(defs.len(), 1, "starlark: assignment x should be defined");
    }

    // ── Thrift ────────────────────────────────────────────────────────────────

    #[test]
    fn test_thrift_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "thrift") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "service Calc { i32 add(1: i32 x, 2: i32 y) }";
        let defs = engine.find_definitions("thrift", src, "x");
        assert_eq!(defs.len(), 1, "thrift: parameter x should be defined");
    }

    #[test]
    fn test_thrift_service_name() {
        let l = loader();
        if skip_if_no(&l, "thrift") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "service Calc { i32 add(1: i32 x) }";
        let defs = engine.find_definitions("thrift", src, "Calc");
        assert_eq!(defs.len(), 1, "thrift: service name Calc should be defined");
    }

    // ── Objective-C ───────────────────────────────────────────────────────────

    #[test]
    fn test_objc_method_parameter() {
        let l = loader();
        if skip_if_no(&l, "objc") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // ObjC method syntax requires @implementation context.
        let src = "@implementation Foo\n- (int)add:(int)x {\n    return x + 1;\n}\n@end";
        let defs = engine.find_definitions("objc", src, "x");
        assert_eq!(defs.len(), 1, "objc: method parameter x should be defined");
        let refs = engine.find_references("objc", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "objc: x reference should resolve");
    }

    // ── Nix ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_nix_function_formal() {
        let l = loader();
        if skip_if_no(&l, "nix") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        // Nix attrset destructuring function: { x, y }: x + y
        let src = "{ x, y }: x + y";
        let defs = engine.find_definitions("nix", src, "x");
        assert_eq!(defs.len(), 1, "nix: formal parameter x should be defined");
    }

    #[test]
    fn test_nix_let_binding() {
        let l = loader();
        if skip_if_no(&l, "nix") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let x = 1; in x + 1";
        let defs = engine.find_definitions("nix", src, "x");
        assert_eq!(defs.len(), 1, "nix: let binding x should be defined");
    }

    // ── ReScript ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rescript_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "rescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let add = (x, y) => x + y";
        let defs = engine.find_definitions("rescript", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "rescript: function parameter x should be defined"
        );
    }

    #[test]
    fn test_rescript_let_binding() {
        let l = loader();
        if skip_if_no(&l, "rescript") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "let x = 1";
        let defs = engine.find_definitions("rescript", src, "x");
        assert_eq!(defs.len(), 1, "rescript: let x should be defined");
    }

    // ── Haskell ───────────────────────────────────────────────────────────────

    #[test]
    fn test_haskell_function_parameter() {
        let l = loader();
        if skip_if_no(&l, "haskell") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "add x y = x + y";
        let defs = engine.find_definitions("haskell", src, "x");
        assert_eq!(
            defs.len(),
            1,
            "haskell: function parameter x should be defined"
        );
        let refs = engine.find_references("haskell", src, "x");
        let resolved = refs.iter().filter(|r| r.definition.is_some()).count();
        assert!(resolved >= 1, "haskell: x reference should resolve");
    }

    #[test]
    fn test_haskell_let_binding() {
        let l = loader();
        if skip_if_no(&l, "haskell") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "f = let x = 1 in x + 1";
        let defs = engine.find_definitions("haskell", src, "x");
        assert_eq!(defs.len(), 1, "haskell: let x should be defined");
    }

    // ── Cap'n Proto ───────────────────────────────────────────────────────────

    #[test]
    fn test_capnp_field_definition() {
        let l = loader();
        if skip_if_no(&l, "capnp") {
            return;
        }
        let engine = ScopeEngine::new(&l);
        let src = "@0xdbb9ad1f14bf0b36;\nstruct Point { x @0 :Float64; y @1 :Float64; }";
        let defs = engine.find_definitions("capnp", src, "x");
        assert_eq!(defs.len(), 1, "capnp: field x should be defined");
    }
}
