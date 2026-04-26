//! Move recipe: relocate a symbol's definition to another file and rewrite imports.
//!
//! Steps:
//! 1. Locate the symbol in the source file
//! 2. Extract its definition text via the editor
//! 3. Append the definition to the destination file
//! 4. Delete the definition from the source file
//! 5. Rewrite import statements in every file that imported it from the old module path
//!    (best-effort: emits a warning and skips when the new path can't be derived)
//! 6. Optionally leave a re-export in the source file (`--reexport`)

use std::path::{Path, PathBuf};

use crate::actions;
use crate::{PlannedEdit, RefactoringContext, RefactoringPlan};

/// Outcome details for a planned move (used by callers that want a richer report
/// than `RefactoringPlan` alone).
pub struct MoveOutcome {
    pub plan: RefactoringPlan,
    pub symbol: String,
    pub from_file: String,
    pub to_file: String,
    pub definition_moved: bool,
    pub import_sites_updated: usize,
    pub import_sites_skipped: usize,
    pub reexport_added: bool,
}

/// Build a move plan without touching the filesystem.
///
/// `from_rel_path` is the relative path to the source file containing the symbol.
/// `to_rel_path` is the relative path to the destination file.
/// `symbol_name` is the unqualified name of the symbol to move.
pub async fn plan_move(
    ctx: &RefactoringContext,
    from_rel_path: &str,
    to_rel_path: &str,
    symbol_name: &str,
    reexport: bool,
) -> Result<MoveOutcome, String> {
    let from_abs = ctx.root.join(from_rel_path);
    let to_abs = ctx.root.join(to_rel_path);

    if from_abs == to_abs {
        return Err(format!(
            "source and destination are the same file: {}",
            from_rel_path
        ));
    }

    let from_content = std::fs::read_to_string(&from_abs)
        .map_err(|e| format!("Error reading {}: {}", from_rel_path, e))?;

    // 1. Locate the symbol in the source.
    let mut loc = actions::locate_symbol(ctx, &from_abs, &from_content, symbol_name)
        .ok_or_else(|| format!("Symbol '{}' not found in {}", symbol_name, from_rel_path))?;

    // Extend the start backward to include leading decorations (doc comments,
    // attributes, decorators) identified via tree-sitter `node.kind()`. Falls
    // back to `loc.start_byte` for languages without a grammar.
    let extended_start = actions::decoration_extended_start(&from_abs, &from_content, &loc);
    loc.start_byte = extended_start;

    // 2. Extract definition text (whole-line span of the symbol).
    let line_start = from_content[..loc.start_byte]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut def_end = loc.end_byte;
    if def_end < from_content.len() && from_content.as_bytes()[def_end] == b'\n' {
        def_end += 1;
    }
    let definition_text = from_content[line_start..def_end].to_string();

    let mut edits: Vec<PlannedEdit> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // 3. Append the definition to the destination file (creating it if absent).
    let dest_original = std::fs::read_to_string(&to_abs).unwrap_or_default();
    let dest_new = ctx.editor.append_to_file(&dest_original, &definition_text);
    edits.push(PlannedEdit {
        file: to_abs.clone(),
        original: dest_original.clone(),
        new_content: dest_new,
        description: format!("append {}", symbol_name),
    });

    // 4. Remove the definition from the source file (and optionally leave a re-export).
    let mut src_new = ctx.editor.delete_symbol(&from_content, &loc);
    let mut reexport_added = false;
    if reexport {
        if let Some(stub) = build_reexport(&from_abs, &to_abs, symbol_name) {
            // Append the re-export at the end of the source file.
            src_new = ctx.editor.append_to_file(&src_new, &stub);
            reexport_added = true;
        } else {
            warnings.push(format!(
                "could not derive re-export for {} (unsupported language)",
                from_rel_path
            ));
        }
    }
    edits.push(PlannedEdit {
        file: from_abs.clone(),
        original: from_content.clone(),
        new_content: src_new,
        description: format!("remove {}", symbol_name),
    });

    // 5. Rewrite import statements in every importer.
    let mut import_sites_updated = 0usize;
    let mut import_sites_skipped = 0usize;

    if let Some(ref idx) = ctx.index {
        let importers = idx
            .find_symbol_importers_with_module(symbol_name)
            .await
            .unwrap_or_default();

        // Group importers by file so each file produces at most one PlannedEdit.
        use std::collections::HashMap;
        let mut by_file: HashMap<String, Vec<(usize, Option<String>)>> = HashMap::new();
        for (file, _name, _alias, line, module) in importers {
            // Skip: the source file itself (we already edited it).
            if file == from_rel_path {
                continue;
            }
            by_file.entry(file).or_default().push((line, module));
        }

        for (rel_path, lines_modules) in by_file {
            let abs_path = ctx.root.join(&rel_path);
            let original = match std::fs::read_to_string(&abs_path) {
                Ok(c) => c,
                Err(_) => {
                    warnings.push(format!("could not read importer file: {}", rel_path));
                    import_sites_skipped += lines_modules.len();
                    continue;
                }
            };

            let mut current = original.clone();
            let mut file_changed = false;
            for (line_no, old_module) in lines_modules {
                let Some(old_module) = old_module else {
                    warnings.push(format!(
                        "{}:{}: import has no module path; skipped",
                        rel_path, line_no
                    ));
                    import_sites_skipped += 1;
                    continue;
                };
                let Some(new_module) = derive_new_module(&abs_path, &to_abs, &old_module) else {
                    warnings.push(format!(
                        "{}:{}: cannot derive new module path for destination {}; skipped",
                        rel_path, line_no, to_rel_path
                    ));
                    import_sites_skipped += 1;
                    continue;
                };
                match replace_module_on_line(&current, line_no, &old_module, &new_module) {
                    Some(updated) => {
                        current = updated;
                        file_changed = true;
                        import_sites_updated += 1;
                    }
                    None => {
                        warnings.push(format!(
                            "{}:{}: could not locate module string '{}' on line; skipped",
                            rel_path, line_no, old_module
                        ));
                        import_sites_skipped += 1;
                    }
                }
            }

            if file_changed {
                edits.push(PlannedEdit {
                    file: abs_path,
                    original,
                    new_content: current,
                    description: format!("rewrite imports of {}", symbol_name),
                });
            }
        }
    } else {
        warnings.push(
            "Index not available; moved definition only (import sites not rewritten)".to_string(),
        );
    }

    Ok(MoveOutcome {
        plan: RefactoringPlan {
            operation: "move".to_string(),
            edits,
            warnings,
        },
        symbol: symbol_name.to_string(),
        from_file: from_rel_path.to_string(),
        to_file: to_rel_path.to_string(),
        definition_moved: true,
        import_sites_updated,
        import_sites_skipped,
        reexport_added,
    })
}

/// Replace `old_module` with `new_module` on a specific 1-based line of `content`.
/// Returns `None` if the substring is not present on that line.
fn replace_module_on_line(
    content: &str,
    line_no: usize,
    old_module: &str,
    new_module: &str,
) -> Option<String> {
    if old_module.is_empty() {
        return None;
    }
    let line_start = byte_offset_for_line(content, line_no);
    let line_end = content[line_start..]
        .find('\n')
        .map(|n| line_start + n)
        .unwrap_or(content.len());
    let line = &content[line_start..line_end];
    if !line.contains(old_module) {
        return None;
    }
    let new_line = line.replacen(old_module, new_module, 1);
    if new_line == line {
        return None;
    }
    let mut out = String::with_capacity(content.len() + new_module.len());
    out.push_str(&content[..line_start]);
    out.push_str(&new_line);
    out.push_str(&content[line_end..]);
    Some(out)
}

fn byte_offset_for_line(content: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    let mut seen = 0usize;
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen == line - 1 {
                return i + 1;
            }
        }
    }
    content.len()
}

/// Derive the new module path string for an `import` in `importer_path` that previously
/// referenced `old_module`. Returns `None` when the language can't be confidently handled.
///
/// Strategy per language:
/// - **Python**: dotted module path from project root (`pkg/sub/mod.py` → `pkg.sub.mod`).
///   We only switch when both the old and new module strings look dotted; otherwise we
///   leave the user a warning so they can fix relative imports by hand.
/// - **Go**: directory-based import path. We replace the trailing module segment chain.
/// - **Rust / JavaScript / TypeScript / others**: skipped (returns `None`). Rust uses
///   `crate::`/`super::` paths that aren't a 1:1 function of file path; JS/TS use
///   relative `./foo` paths whose form depends on the importer's location and the
///   project's module resolution. Honest "I don't know" beats wrong output.
fn derive_new_module(importer_path: &Path, dest_path: &Path, old_module: &str) -> Option<String> {
    let ext = dest_path.extension().and_then(|e| e.to_str())?;
    match ext {
        "py" => derive_python_module(dest_path, old_module),
        "go" => Some(go_import_path(dest_path)?),
        "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" => derive_js_relative(importer_path, dest_path),
        _ => None,
    }
}

fn derive_python_module(dest_path: &Path, old_module: &str) -> Option<String> {
    // Only handle dotted (absolute) module strings — relative imports (".foo") need
    // knowledge of the importer's package layout we don't have here.
    if old_module.starts_with('.') {
        return None;
    }
    // Walk up from the destination collecting components until we hit a directory
    // that does NOT contain an `__init__.py` (i.e. the package root sibling).
    let stem = dest_path.file_stem()?.to_str()?;
    let mut parts: Vec<String> = Vec::new();
    if stem != "__init__" {
        parts.push(stem.to_string());
    }
    let mut dir = dest_path.parent()?;
    while dir.join("__init__.py").exists() {
        let name = dir.file_name()?.to_str()?.to_string();
        parts.push(name);
        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }
    if parts.is_empty() {
        return None;
    }
    parts.reverse();
    Some(parts.join("."))
}

fn go_import_path(dest_path: &Path) -> Option<String> {
    // Go imports the directory, not the file. Best we can do without parsing
    // go.mod is the directory components — caller still substitutes the previous
    // import string, so we only need a stable form.
    let dir = dest_path.parent()?;
    let s = dir.to_str()?;
    Some(s.to_string())
}

fn derive_js_relative(importer_path: &Path, dest_path: &Path) -> Option<String> {
    // Compute a relative path from the importer's directory to the destination,
    // stripping the file extension (the standard JS/TS convention).
    let importer_dir = importer_path.parent()?;
    let rel = pathdiff(dest_path, importer_dir)?;
    let rel_str = rel.to_str()?;
    let without_ext = match rel_str.rsplit_once('.') {
        Some((stem, "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx")) => stem,
        _ => rel_str,
    };
    let with_prefix = if without_ext.starts_with('.') || without_ext.starts_with('/') {
        without_ext.to_string()
    } else {
        format!("./{}", without_ext)
    };
    Some(with_prefix)
}

/// Minimal `pathdiff::diff_paths` reimplementation (no external dep).
fn pathdiff(target: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let target: Vec<Component> = target.components().collect();
    let base: Vec<Component> = base.components().collect();
    let common = target
        .iter()
        .zip(base.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let ups = base.len() - common;
    let mut out = PathBuf::new();
    if ups == 0 {
        out.push(".");
    }
    for _ in 0..ups {
        out.push("..");
    }
    for c in &target[common..] {
        out.push(c.as_os_str());
    }
    Some(out)
}

/// Build a re-export stub left at the source location after a move.
/// Returns `None` for languages where we don't have a defensible default.
fn build_reexport(from_path: &Path, to_path: &Path, symbol: &str) -> Option<String> {
    let ext = from_path.extension().and_then(|e| e.to_str())?;
    match ext {
        "py" => {
            let module = derive_python_module(to_path, symbol)?;
            Some(format!("from {} import {}\n", module, symbol))
        }
        // Rust re-exports require knowing the canonical module path of the destination,
        // which is not a function of file path alone (mod tree may differ from FS layout).
        // Be honest: don't fabricate a `pub use`.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_module_on_line_basic() {
        let content = "from old.path import Thing\nother\n";
        let out = replace_module_on_line(content, 1, "old.path", "new.path").unwrap();
        assert_eq!(out, "from new.path import Thing\nother\n");
    }

    #[test]
    fn replace_module_on_line_missing_returns_none() {
        let content = "from x import Thing\n";
        assert!(replace_module_on_line(content, 1, "absent", "y").is_none());
    }

    #[test]
    fn pathdiff_sibling() {
        let target = Path::new("a/b/c.ts");
        let base = Path::new("a/b");
        assert_eq!(pathdiff(target, base).unwrap(), PathBuf::from("./c.ts"));
    }

    #[tokio::test]
    async fn plan_move_includes_python_decorator_and_comment() {
        if normalize_languages::parsers::parser_for("python").is_none() {
            eprintln!("skipping: python grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let from_path = dir.path().join("src.py");
        let to_path = dir.path().join("dest.py");
        let from_content = "\
import os

# Important note about my_func.
@decorator
def my_func():
    return 1

def other():
    return 2
";
        std::fs::write(&from_path, from_content).unwrap();
        std::fs::write(&to_path, "").unwrap();

        let ctx = RefactoringContext {
            root: dir.path().to_path_buf(),
            editor: normalize_edit::Editor::new(),
            index: None,
            loader: normalize_languages::GrammarLoader::new(),
        };

        let outcome = plan_move(&ctx, "src.py", "dest.py", "my_func", false)
            .await
            .expect("plan_move");

        // The destination file edit should contain the leading comment and decorator.
        let dest_edit = outcome
            .plan
            .edits
            .iter()
            .find(|e| e.file == to_path)
            .expect("dest edit");
        assert!(
            dest_edit
                .new_content
                .contains("# Important note about my_func."),
            "dest missing leading comment; got: {:?}",
            dest_edit.new_content
        );
        assert!(
            dest_edit.new_content.contains("@decorator"),
            "dest missing decorator; got: {:?}",
            dest_edit.new_content
        );

        // The source file edit should have removed the comment and decorator.
        let src_edit = outcome
            .plan
            .edits
            .iter()
            .find(|e| e.file == from_path)
            .expect("src edit");
        assert!(
            !src_edit.new_content.contains("@decorator"),
            "src still contains decorator; got: {:?}",
            src_edit.new_content
        );
        assert!(
            !src_edit
                .new_content
                .contains("# Important note about my_func."),
            "src still contains leading comment; got: {:?}",
            src_edit.new_content
        );
        // `other` should remain.
        assert!(src_edit.new_content.contains("def other():"));
    }

    #[test]
    fn derive_js_relative_strips_ext() {
        let importer = Path::new("src/app.ts");
        let dest = Path::new("src/lib/foo.ts");
        let out = derive_js_relative(importer, dest).unwrap();
        assert_eq!(out, "./lib/foo");
    }
}
