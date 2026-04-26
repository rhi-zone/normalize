//! Semantic actions: query and mutation primitives for refactoring recipes.
//!
//! **Query actions** return data without side effects.
//! **Mutation actions** produce `PlannedEdit`s without touching the filesystem.

use std::collections::HashSet;
use std::path::Path;

use normalize_edit::SymbolLocation;
use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::support_for_path;
use tree_sitter::StreamingIterator as _;

use crate::{CallerRef, ImportRef, PlannedEdit, RefactoringContext, References};

// ── Query actions ────────────────────────────────────────────────────

/// Find a symbol's location in a file.
pub fn locate_symbol(
    ctx: &RefactoringContext,
    file: &Path,
    content: &str,
    name: &str,
) -> Option<SymbolLocation> {
    ctx.editor.find_symbol(file, content, name, false)
}

/// Tree-sitter node `kind` values that count as a leading decoration
/// (doc comment, attribute, decorator, annotation, pragma) for any language.
///
/// Comment kinds are matched separately by substring: any kind containing
/// `"comment"` is treated as a decoration. Listed here are the non-comment
/// kinds across the grammars normalize supports.
const DECORATION_KINDS: &[&str] = &[
    "attribute_item",       // Rust outer attribute `#[...]`
    "inner_attribute_item", // Rust inner attribute `#![...]`
    "meta_item",            // Rust attribute body
    "attribute",            // C#, generic
    "attribute_list",       // C#
    "decorator",            // Python, JS/TS (TypeScript decorators)
    "decorator_list",       // grouped decorators
    "annotation",           // Java, Kotlin
    "marker_annotation",    // Java
    "modifiers",            // Java/Kotlin annotations live under `modifiers`
    "pragma",               // C/C++
    "preproc_call",         // C/C++ preprocessor lines like `#pragma`
];

fn is_decoration_kind(kind: &str) -> bool {
    kind.contains("comment") || DECORATION_KINDS.contains(&kind)
}

/// Walk backward from the symbol's node through preceding named siblings,
/// collecting decoration nodes (doc comments, attributes, decorators, etc.).
/// Returns `(byte_offset, warning)` where:
/// - `byte_offset` is the line-start of the earliest decoration found, or
///   `loc.start_byte` if there are no decorations or no grammar is available.
/// - `warning` is `Some(msg)` when the function fell back because the grammar
///   was unavailable; `None` when the grammar was used (even if no decorations
///   were found).
///
/// Classification is by `node.kind()` from the grammar — never by source text.
pub fn decoration_extended_start(
    file: &Path,
    content: &str,
    loc: &SymbolLocation,
) -> (usize, Option<String>) {
    let fallback = loc.start_byte;
    let Some(support) = support_for_path(file) else {
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("<unknown>");
        return (
            fallback,
            Some(format!(
                "No language support for {ext}: doc comments and attributes will not be included with the moved symbol"
            )),
        );
    };
    let grammar = support.grammar_name();
    let Some(tree) = parse_with_grammar(grammar, content) else {
        return (
            fallback,
            Some(format!(
                "Grammar for {grammar} not loaded: doc comments and attributes will not be included. Install grammars with `normalize grammars install`."
            )),
        );
    };

    let root = tree.root_node();
    // The symbol's def node — descendant_for_byte_range expects start <= end and
    // both within the tree. Use the line-start of the symbol as the anchor.
    let sym_start = loc.start_byte.min(content.len());
    let sym_end = loc.end_byte.min(content.len()).max(sym_start);
    let Some(mut node) = root.descendant_for_byte_range(sym_start, sym_end) else {
        return (fallback, None);
    };

    // descendant_for_byte_range may return a small inner node (e.g. an identifier)
    // when the symbol's start byte is line-aligned. Walk up to the outermost
    // ancestor whose start_byte equals the matched node's start_byte — this is
    // the def/declaration node we want preceding-sibling info for.
    while let Some(parent) = node.parent() {
        if parent.start_byte() == node.start_byte() && parent.id() != root.id() {
            node = parent;
        } else {
            break;
        }
    }

    // Build the set of decoration node IDs using the decorations query when
    // available, falling back to the hardcoded kind list otherwise.
    let loader = grammar_loader();
    let decoration_ids: Option<HashSet<usize>> = loader.get_decorations(grammar).and_then(|q| {
        let compiled = loader.get_compiled_query(grammar, "decorations", &q)?;
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&compiled, root, content.as_bytes());
        let mut ids = HashSet::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                ids.insert(capture.node.id());
            }
        }
        Some(ids)
    });

    let is_decoration = |n: tree_sitter::Node<'_>| -> bool {
        if let Some(ref ids) = decoration_ids {
            ids.contains(&n.id())
        } else {
            is_decoration_kind(n.kind())
        }
    };

    // Walk preceding named siblings while they classify as decorations.
    let mut earliest_start = node.start_byte();
    let mut cursor = node;
    while let Some(prev) = cursor.prev_named_sibling() {
        if !is_decoration(prev) {
            break;
        }
        // Only include if the gap between `prev` and the decoration block we've
        // already accepted is whitespace-only (no intervening code/punctuation).
        let gap = &content.as_bytes()[prev.end_byte()..earliest_start];
        if !gap.iter().all(|b| b.is_ascii_whitespace()) {
            break;
        }
        earliest_start = prev.start_byte();
        cursor = prev;
    }

    if earliest_start == node.start_byte() {
        return (fallback, None);
    }

    // Snap to the start of the line containing earliest_start so we capture
    // any indentation on that line (consistent with `delete_symbol`'s line
    // semantics).
    let snapped = content[..earliest_start]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    (snapped, None)
}

/// Find all cross-file references to a symbol (callers + importers).
///
/// Returns empty references if no index is available.
pub async fn find_references(
    ctx: &RefactoringContext,
    symbol_name: &str,
    def_file: &str,
) -> References {
    let Some(ref idx) = ctx.index else {
        return References {
            callers: vec![],
            importers: vec![],
        };
    };

    let callers = idx
        .find_callers(symbol_name, def_file)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(file, caller, line, access)| CallerRef {
            file,
            caller,
            line,
            access,
        })
        .collect();

    let importers = idx
        .find_symbol_importers(symbol_name)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(file, name, alias, line)| ImportRef {
            file,
            name,
            alias,
            line,
        })
        .collect();

    References { callers, importers }
}

/// Check for naming conflicts that a rename would introduce.
///
/// Returns a list of conflict descriptions (empty = no conflicts).
pub async fn check_conflicts(
    ctx: &RefactoringContext,
    def_file: &Path,
    def_content: &str,
    new_name: &str,
    importers: &[ImportRef],
) -> Vec<String> {
    let mut conflicts = vec![];

    // 1. Does new_name already exist as a symbol in the definition file?
    if ctx
        .editor
        .find_symbol(def_file, def_content, new_name, false)
        .is_some()
    {
        let rel = def_file
            .strip_prefix(&ctx.root)
            .unwrap_or(def_file)
            .to_string_lossy();
        conflicts.push(format!("{}: symbol '{}' already exists", rel, new_name));
    }

    // 2. Does any importer file already import something named new_name?
    if !importers.is_empty()
        && let Some(ref idx) = ctx.index
    {
        for imp in importers {
            if idx
                .has_import_named(&imp.file, new_name)
                .await
                .unwrap_or(false)
            {
                conflicts.push(format!("{}: already imports '{}'", imp.file, new_name));
            }
        }
    }

    conflicts
}

// ── Mutation actions ─────────────────────────────────────────────────

/// Plan renames of an identifier across specific lines in a file.
///
/// Groups all line-level renames into a single `PlannedEdit` for the file.
/// Returns `None` if no lines actually matched (e.g. stale index data).
pub fn plan_rename_in_file(
    ctx: &RefactoringContext,
    file: &Path,
    content: &str,
    lines: &[usize],
    old_name: &str,
    new_name: &str,
) -> Option<PlannedEdit> {
    let mut current = content.to_string();
    let mut changed = false;

    for &line_no in lines {
        if let Some(new_content) = ctx
            .editor
            .rename_identifier_in_line(&current, line_no, old_name, new_name)
        {
            current = new_content;
            changed = true;
        }
    }

    if changed {
        Some(PlannedEdit {
            file: file.to_path_buf(),
            original: content.to_string(),
            new_content: current,
            description: format!("{} -> {}", old_name, new_name),
        })
    } else {
        None
    }
}

/// Plan deletion of a symbol from a file.
pub fn plan_delete_symbol(
    ctx: &RefactoringContext,
    file: &Path,
    content: &str,
    loc: &SymbolLocation,
) -> PlannedEdit {
    let new_content = ctx.editor.delete_symbol(content, loc);
    PlannedEdit {
        file: file.to_path_buf(),
        original: content.to_string(),
        new_content,
        description: format!("delete {}", loc.name),
    }
}

/// Plan insertion of code relative to a symbol.
pub fn plan_insert(
    ctx: &RefactoringContext,
    file: &Path,
    content: &str,
    loc: &SymbolLocation,
    position: InsertPosition,
    code: &str,
) -> PlannedEdit {
    let new_content = match position {
        InsertPosition::Before => ctx.editor.insert_before(content, loc, code),
        InsertPosition::After => ctx.editor.insert_after(content, loc, code),
    };
    let pos_str = match position {
        InsertPosition::Before => "before",
        InsertPosition::After => "after",
    };
    PlannedEdit {
        file: file.to_path_buf(),
        original: content.to_string(),
        new_content,
        description: format!("insert {} {}", pos_str, loc.name),
    }
}

/// Plan replacement of a symbol's content.
pub fn plan_replace_symbol(
    ctx: &RefactoringContext,
    file: &Path,
    content: &str,
    loc: &SymbolLocation,
    new_code: &str,
) -> PlannedEdit {
    let new_content = ctx.editor.replace_symbol(content, loc, new_code);
    PlannedEdit {
        file: file.to_path_buf(),
        original: content.to_string(),
        new_content,
        description: format!("replace {}", loc.name),
    }
}

/// Where to insert code relative to a symbol.
pub enum InsertPosition {
    Before,
    After,
}

#[cfg(test)]
mod tests {
    use super::*;
    use normalize_edit::Editor;

    fn make_ctx(root: &Path) -> RefactoringContext {
        RefactoringContext {
            root: root.to_path_buf(),
            editor: Editor::new(),
            index: None,
            loader: normalize_languages::GrammarLoader::new(),
        }
    }

    #[test]
    fn plan_rename_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path());
        let file = dir.path().join("test.rs");
        let content = "fn old_func() {}\nfn other() { old_func(); }\n";

        let edit = plan_rename_in_file(&ctx, &file, content, &[1], "old_func", "new_func");
        assert!(edit.is_some());
        let edit = edit.unwrap();
        assert!(edit.new_content.contains("new_func"));
        assert!(edit.new_content.contains("old_func")); // line 2 not renamed
    }

    #[test]
    fn plan_rename_multiple_lines() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path());
        let file = dir.path().join("test.rs");
        let content = "fn old_func() {}\nfn other() { old_func(); }\n";

        let edit = plan_rename_in_file(&ctx, &file, content, &[1, 2], "old_func", "new_func");
        assert!(edit.is_some());
        let edit = edit.unwrap();
        assert!(!edit.new_content.contains("old_func"));
    }

    #[test]
    fn plan_rename_no_match_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path());
        let file = dir.path().join("test.rs");
        let content = "fn something() {}\n";

        let edit = plan_rename_in_file(&ctx, &file, content, &[1], "nonexistent", "new_name");
        assert!(edit.is_none());
    }

    #[test]
    fn locate_symbol_found() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path());
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "fn my_func() {}\n").unwrap();

        let loc = locate_symbol(&ctx, &file, "fn my_func() {}\n", "my_func");
        assert!(loc.is_some());
        assert_eq!(loc.unwrap().name, "my_func");
    }

    /// Returns true if the named external grammar can be loaded; tests that need
    /// a grammar should `return` early when this is false to avoid spurious failures
    /// in environments without `NORMALIZE_GRAMMAR_PATH` configured.
    fn grammar_available(name: &str) -> bool {
        normalize_languages::parsers::parser_for(name).is_some()
    }

    #[test]
    fn decoration_python_decorator_and_comment() {
        if !grammar_available("python") {
            eprintln!("skipping: python grammar not available");
            return;
        }
        let content = "\
import x

# Leading comment line 1.
# Leading comment line 2.
@decorator
@other_decorator
def my_func():
    pass
";
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        let editor = normalize_edit::Editor::new();
        std::fs::write(&file, content).unwrap();
        let loc = editor
            .find_symbol(&file, content, "my_func", false)
            .expect("locate");
        let (start, warning) = decoration_extended_start(&file, content, &loc);
        assert!(warning.is_none(), "unexpected warning: {:?}", warning);
        let slice = &content[start..];
        assert!(
            slice.starts_with("# Leading comment line 1.\n"),
            "expected leading comments + decorators included; got: {:?}",
            slice
        );
        assert!(slice.contains("@decorator\n"));
        assert!(slice.contains("@other_decorator\n"));
    }

    #[test]
    fn decoration_python_no_decoration_returns_original() {
        if !grammar_available("python") {
            eprintln!("skipping: python grammar not available");
            return;
        }
        let content = "def alone():\n    pass\n";
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        std::fs::write(&file, content).unwrap();
        let editor = normalize_edit::Editor::new();
        let loc = editor
            .find_symbol(&file, content, "alone", false)
            .expect("locate");
        let (start, warning) = decoration_extended_start(&file, content, &loc);
        assert!(warning.is_none(), "unexpected warning: {:?}", warning);
        assert_eq!(start, loc.start_byte);
    }

    #[test]
    fn decoration_javascript_decorator() {
        if !grammar_available("javascript") {
            eprintln!("skipping: javascript grammar not available");
            return;
        }
        let content = "\
// Leading comment.
class Wrapper {
  @log
  myMethod() {}
}
";
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.js");
        std::fs::write(&file, content).unwrap();
        let editor = normalize_edit::Editor::new();
        let loc = editor
            .find_symbol(&file, content, "myMethod", false)
            .expect("locate");
        let (start, warning) = decoration_extended_start(&file, content, &loc);
        assert!(warning.is_none(), "unexpected warning: {:?}", warning);
        // The decorator and the line above (whitespace-only indent) must be included.
        let slice = &content[start..];
        assert!(
            slice.trim_start().starts_with("@log"),
            "expected @log decorator included; got: {:?}",
            slice
        );
    }

    #[test]
    fn decoration_unsupported_language_falls_back() {
        // Path with no registered grammar — should return loc.start_byte unchanged.
        let content = "anything here";
        let file = std::path::PathBuf::from("test.unknown_ext_xyz");
        let loc = SymbolLocation {
            name: "x".to_string(),
            kind: "function".to_string(),
            start_byte: 5,
            end_byte: 10,
            start_line: 1,
            end_line: 1,
            indent: String::new(),
        };
        let (start, warning) = decoration_extended_start(&file, content, &loc);
        assert_eq!(start, 5);
        assert!(
            warning.is_some(),
            "expected a warning for unsupported language"
        );
        assert!(
            warning.unwrap().contains("unknown_ext_xyz"),
            "warning should mention the extension"
        );
    }

    #[test]
    fn locate_symbol_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path());
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "fn my_func() {}\n").unwrap();

        let loc = locate_symbol(&ctx, &file, "fn my_func() {}\n", "nonexistent");
        assert!(loc.is_none());
    }
}
