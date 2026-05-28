//! Tree-sitter-based documentation extraction from a source tree.
//!
//! Given an extracted package source directory (see [`crate::source_archive`]),
//! this module locates the source file that defines a requested symbol, parses it
//! with the appropriate tree-sitter grammar, walks to the symbol's definition node,
//! and extracts its docstring and signature via the [`normalize_languages::Language`]
//! trait.
//!
//! This is the shared core consumed by per-ecosystem doc extractors (Go, Python).
//! The file-location heuristic is intentionally generic: it walks every file in the
//! tree that the grammar supports, parses each, and searches for a node whose name
//! matches the last segment of `symbol_path`. It prefers a genuine definition node
//! over an import/re-export/alias that merely shadows the name (see
//! [`normalize_languages::Language::is_definition_node`]) — e.g. it skips
//! `from .sessions import Session` in a package `__init__.py` to find the real
//! `class Session` in `sessions.py` — falling back to a name match only if no
//! definition exists anywhere in the tree. A caller that can locate the file cheaply
//! (e.g. from a package's directory layout) should narrow `dir` to that subtree
//! before calling.

use crate::DocsError;
use crate::symbol_docs::{DocFormat, SymbolDoc};
use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::{Language, support_for_grammar};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Extract documentation for `symbol_path` from an extracted source tree.
///
/// - `dir`: root of the extracted package source.
/// - `grammar`: tree-sitter grammar name (e.g. `"go"`, `"python"`). Must be a
///   grammar that has a registered [`Language`] impl and is supported by the parser.
/// - `package`, `version`: identifying metadata copied verbatim into the result.
/// - `symbol_path`: dotted/slashed path; only the **last segment** is matched against
///   definition node names. The full string is preserved in `SymbolDoc::symbol_path`.
///
/// The resulting [`SymbolDoc`] carries `doc_format: DocFormat::PlainText` — we do not
/// interpret RST/Google/NumPy docstring conventions, so the raw extracted text is
/// reported as plain text. `language` is set to the grammar name.
pub fn extract_from_source_tree(
    dir: &Path,
    grammar: &str,
    package: &str,
    symbol_path: &str,
    version: &str,
) -> Result<SymbolDoc, DocsError> {
    let lang = support_for_grammar(grammar).ok_or_else(|| {
        DocsError::NotFound(format!("no language support for grammar '{}'", grammar))
    })?;

    let target_name = symbol_path
        .rsplit(['.', ':', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DocsError::NotFound(format!("empty symbol path '{}'", symbol_path)))?;

    let files = collect_supported_files(dir, lang);
    if files.is_empty() {
        return Err(DocsError::NotFound(format!(
            "no '{}' source files under {}",
            grammar,
            dir.display()
        )));
    }

    // A name match on an import/re-export/alias node (e.g. Python's
    // `from .sessions import Session`) shadows the real definition, which may live
    // in a different file. Prefer the first genuine definition found anywhere in the
    // tree; only fall back to a non-definition name match if no definition exists.
    let mut fallback: Option<SymbolMatch> = None;

    for file in &files {
        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };
        let Some(tree) = parse_with_grammar(grammar, &content) else {
            continue;
        };
        match find_symbol(lang, &tree.root_node(), &content, target_name) {
            Some(m) if m.is_definition => {
                return Ok(into_doc(m, grammar, package, symbol_path, version));
            }
            Some(m) => {
                fallback.get_or_insert(m);
            }
            None => {}
        }
    }

    if let Some(m) = fallback {
        return Ok(into_doc(m, grammar, package, symbol_path, version));
    }

    Err(DocsError::NotFound(format!(
        "symbol '{}' not found in source tree {}",
        symbol_path,
        dir.display()
    )))
}

/// Recursively collect all files under `dir` whose extension the language supports.
fn collect_supported_files(dir: &Path, lang: &dyn Language) -> Vec<PathBuf> {
    let exts = lang.extensions();
    let mut out = Vec::new();
    collect_inner(dir, exts, &mut out);
    out
}

fn collect_inner(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && exts.contains(&ext)
            {
                out.push(path);
            }
        } else if path.is_dir() {
            subdirs.push(path);
        }
    }
    for sub in subdirs {
        collect_inner(&sub, exts, out);
    }
}

/// A name match found while walking a parsed file.
struct SymbolMatch {
    /// Tree-sitter node kind (e.g. `"function_declaration"`).
    kind: String,
    signature: String,
    doc_body: Option<String>,
    /// Whether the matched node is a genuine definition (vs. an import/re-export).
    is_definition: bool,
}

/// Assemble a [`SymbolDoc`] from a matched node and the call's metadata.
fn into_doc(
    m: SymbolMatch,
    grammar: &str,
    package: &str,
    symbol_path: &str,
    version: &str,
) -> SymbolDoc {
    let target_name = symbol_path
        .rsplit(['.', ':', '/'])
        .next()
        .unwrap_or(symbol_path);
    SymbolDoc {
        name: target_name.to_string(),
        language: grammar.to_string(),
        package: package.to_string(),
        version: version.to_string(),
        symbol_path: symbol_path.to_string(),
        kind: m.kind,
        signature: Some(m.signature),
        doc_body: m.doc_body.unwrap_or_default(),
        doc_format: DocFormat::PlainText,
        examples: vec![],
        source_url: String::new(),
        fetched_at: chrono::Utc::now(),
    }
}

/// Walk the tree for a node named `target_name`.
///
/// Returns the first name match in this file, preferring a genuine definition: if a
/// definition node is found it is returned immediately; otherwise the first
/// non-definition match (e.g. an import/re-export) is returned so callers can use it
/// as a fallback. `kind` is the tree-sitter node kind (e.g. `"function_declaration"`).
fn find_symbol(
    lang: &dyn Language,
    node: &Node,
    content: &str,
    target_name: &str,
) -> Option<SymbolMatch> {
    let mut fallback: Option<SymbolMatch> = None;

    if lang.node_name(node, content) == Some(target_name) {
        let m = SymbolMatch {
            kind: node.kind().to_string(),
            signature: lang.build_signature(node, content),
            doc_body: lang.extract_docstring(node, content),
            is_definition: lang.is_definition_node(node),
        };
        if m.is_definition {
            return Some(m);
        }
        fallback = Some(m);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_symbol(lang, &child, content, target_name) {
            if found.is_definition {
                return Some(found);
            }
            fallback.get_or_insert(found);
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(name: &str, content: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(name), content).unwrap();
        tmp
    }

    #[test]
    fn extracts_go_function_doc_and_signature() {
        let src = "package foo\n\
\n\
// Greet returns a friendly greeting for the given name.\n\
func Greet(name string) string {\n\
\treturn \"hi \" + name\n\
}\n";
        let tmp = write_fixture("foo.go", src);
        let doc =
            extract_from_source_tree(tmp.path(), "go", "example.com/foo", "foo.Greet", "v1.0.0")
                .expect("should extract Greet");
        assert_eq!(doc.name, "Greet");
        assert_eq!(doc.language, "go");
        assert_eq!(doc.package, "example.com/foo");
        assert_eq!(doc.version, "v1.0.0");
        assert_eq!(doc.doc_format, DocFormat::PlainText);
        assert!(
            doc.signature.as_deref().unwrap().contains("Greet"),
            "signature: {:?}",
            doc.signature
        );
        assert!(
            doc.doc_body.contains("friendly greeting"),
            "doc_body: {:?}",
            doc.doc_body
        );
    }

    #[test]
    fn prefers_real_definition_over_python_reexport() {
        // A package whose __init__.py re-exports `Session` from a submodule. The walk
        // hits __init__.py first (alphabetically), but its match is an
        // `import_from_statement` re-export with no body — the real `class Session`
        // with the docstring lives in sessions.py. We must return the latter.
        let tmp = tempfile::TempDir::new().unwrap();
        let pkg = tmp.path().join("requests");
        std::fs::create_dir(&pkg).unwrap();
        std::fs::write(pkg.join("__init__.py"), "from .sessions import Session\n").unwrap();
        std::fs::write(
            pkg.join("sessions.py"),
            "class Session:\n    \"\"\"A Requests session.\"\"\"\n    pass\n",
        )
        .unwrap();

        let doc = extract_from_source_tree(
            tmp.path(),
            "python",
            "requests",
            "requests.Session",
            "2.0.0",
        )
        .expect("should extract Session");
        assert_eq!(doc.name, "Session");
        assert_eq!(doc.kind, "class_definition", "kind: {:?}", doc.kind);
        assert!(
            doc.signature.as_deref().unwrap().contains("class Session"),
            "signature: {:?}",
            doc.signature
        );
        assert!(
            doc.doc_body.contains("Requests session"),
            "doc_body should come from sessions.py, got: {:?}",
            doc.doc_body
        );
    }

    #[test]
    fn falls_back_to_name_match_when_no_definition() {
        // If the only thing named `Session` anywhere is a re-export (no real
        // definition in the tree), we still resolve it rather than 404 — a symbol
        // that genuinely has no local definition should not vanish.
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("__init__.py"),
            "from external.sessions import Session\n",
        )
        .unwrap();
        let doc = extract_from_source_tree(tmp.path(), "python", "p", "p.Session", "v0").unwrap();
        assert_eq!(doc.name, "Session");
        // The fallback match is the import node, not a class.
        assert_eq!(doc.kind, "import_from_statement");
    }

    #[test]
    fn missing_symbol_is_not_found() {
        let tmp = write_fixture("foo.go", "package foo\nfunc Bar() {}\n");
        let err = extract_from_source_tree(tmp.path(), "go", "p", "foo.Nope", "v0").unwrap_err();
        assert!(matches!(err, DocsError::NotFound(_)));
    }
}
