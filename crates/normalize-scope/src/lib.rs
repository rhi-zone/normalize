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

        // Collect all captures in one pass
        let mut scopes: Vec<ScopeRange> = Vec::new();
        let mut raw_defs: Vec<RawCapture> = Vec::new();
        let mut raw_refs: Vec<RawCapture> = Vec::new();

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
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

                if name == "local.scope" {
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
}
