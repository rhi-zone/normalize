//! Semantic actions: query and mutation primitives for refactoring recipes.
//!
//! **Query actions** return data without side effects.
//! **Mutation actions** produce `PlannedEdit`s without touching the filesystem.

use std::path::Path;

use normalize_edit::SymbolLocation;

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
