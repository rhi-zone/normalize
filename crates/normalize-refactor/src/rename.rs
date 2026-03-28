//! Rename recipe: composable rename using semantic actions.
//!
//! Decomposes the monolithic `do_rename` into:
//! 1. `locate_symbol` — find definition
//! 2. `find_references` — gather callers + importers from index
//! 3. `check_conflicts` — detect naming collisions
//! 4. `plan_rename_in_file` — produce edits for each affected file

use std::collections::HashMap;

use crate::actions;
use crate::{PlannedEdit, RefactoringContext, RefactoringPlan};

/// Build a rename plan without touching the filesystem.
///
/// Returns a `RefactoringPlan` containing all edits needed, or an error
/// if the target can't be resolved or conflicts are detected.
pub async fn plan_rename(
    ctx: &RefactoringContext,
    def_rel_path: &str,
    old_name: &str,
    new_name: &str,
    force: bool,
) -> Result<RefactoringPlan, String> {
    let def_rel_path = def_rel_path.to_string();
    let def_abs_path = ctx.root.join(&def_rel_path);

    let def_content = std::fs::read_to_string(&def_abs_path)
        .map_err(|e| format!("Error reading {}: {}", def_rel_path, e))?;

    // 1. Locate definition
    let loc = actions::locate_symbol(ctx, &def_abs_path, &def_content, old_name)
        .ok_or_else(|| format!("Symbol '{}' not found in {}", old_name, def_rel_path))?;

    // 2. Find cross-file references
    let refs = actions::find_references(ctx, old_name, &def_rel_path).await;

    // 3. Check conflicts
    if !force {
        let conflicts =
            actions::check_conflicts(ctx, &def_abs_path, &def_content, new_name, &refs.importers)
                .await;
        if !conflicts.is_empty() {
            let detail = conflicts
                .iter()
                .map(|c| format!("  {}", c))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!(
                "Rename '{}' → '{}' would cause conflicts (use --force to override):\n{}",
                old_name, new_name, detail
            ));
        }
    }

    let mut edits: Vec<PlannedEdit> = vec![];
    let mut warnings: Vec<String> = vec![];

    // 4a. Rename in definition file
    if let Some(edit) = actions::plan_rename_in_file(
        ctx,
        &def_abs_path,
        &def_content,
        &[loc.start_line],
        old_name,
        new_name,
    ) {
        edits.push(edit);
    }

    // 4b. Rename at call sites (grouped by file)
    let mut callers_by_file: HashMap<String, Vec<usize>> = HashMap::new();
    for caller in &refs.callers {
        callers_by_file
            .entry(caller.file.clone())
            .or_default()
            .push(caller.line);
    }

    // Track which files we've already produced edits for
    let mut edited_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    edited_files.insert(def_rel_path.clone());

    for (rel_path, lines) in &callers_by_file {
        if rel_path == &def_rel_path {
            // Definition file already handled; self-recursive calls are on the same lines
            continue;
        }
        let abs_path = ctx.root.join(rel_path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => {
                warnings.push(format!("Could not read caller file: {}", rel_path));
                continue;
            }
        };
        if let Some(edit) =
            actions::plan_rename_in_file(ctx, &abs_path, &content, lines, old_name, new_name)
        {
            edits.push(edit);
            edited_files.insert(rel_path.clone());
        }
    }

    // 4c. Rename in import statements (grouped by file)
    let mut importers_by_file: HashMap<String, Vec<usize>> = HashMap::new();
    for imp in &refs.importers {
        importers_by_file
            .entry(imp.file.clone())
            .or_default()
            .push(imp.line);
    }

    for (rel_path, lines) in &importers_by_file {
        if edited_files.contains(rel_path) {
            // File already has an edit — we need to apply import renames on top of
            // the already-renamed content. Find the existing edit and update it.
            if let Some(existing) = edits.iter_mut().find(|e| e.file == ctx.root.join(rel_path)) {
                // Apply import renames on top of the already-modified content
                let mut current = existing.new_content.clone();
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
                    existing.new_content = current;
                }
                continue;
            }
        }
        let abs_path = ctx.root.join(rel_path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => {
                warnings.push(format!("Could not read importer file: {}", rel_path));
                continue;
            }
        };
        if let Some(edit) =
            actions::plan_rename_in_file(ctx, &abs_path, &content, lines, old_name, new_name)
        {
            edits.push(edit);
        }
    }

    // Add warning if no index was available
    if ctx.index.is_none() && refs.callers.is_empty() && refs.importers.is_empty() {
        warnings.push("Index not available; renamed definition only".to_string());
    }

    Ok(RefactoringPlan {
        operation: "rename".to_string(),
        edits,
        warnings,
    })
}
