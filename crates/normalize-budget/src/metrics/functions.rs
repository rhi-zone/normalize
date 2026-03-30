//! Functions added/removed diff metric.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops;
use normalize_facts::Extractor;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

/// Functions/methods introduced or removed.
///
/// Compares function symbol lists at `base_ref` vs the working tree.
/// Returns a measurement with `added=1.0` for added functions and `removed=1.0`
/// for removed functions.
pub struct FunctionDeltaMetric;

impl DiffMetric for FunctionDeltaMetric {
    fn name(&self) -> &'static str {
        "functions"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        symbol_diff(root, base_ref, &["function", "method"])
    }
}

/// Collect symbols of the given kinds from all files in the git tree at `git_ref`.
///
/// Uses gix to read file blobs directly from the object store — no filesystem checkout.
fn collect_symbols_at_ref(
    root: &Path,
    git_ref: &str,
    kinds: &[&str],
) -> anyhow::Result<HashMap<String, ()>> {
    let repo = git_ops::open_repo(root)?;
    let extractor = Extractor::new();
    let mut map = HashMap::new();

    git_ops::walk_tree_at_ref(root, git_ref, |rel_path, blob_id| {
        let Some(content) = git_ops::read_blob_text(&repo, blob_id) else {
            return;
        };
        // Build a fake absolute path for language detection; content is from the blob.
        let abs_path = root.join(rel_path);
        let result = extractor.extract(&abs_path, &content);
        collect_recursive(&result.symbols, rel_path, None, kinds, &mut map);
    })?;

    Ok(map)
}

/// Collect symbols from the current working tree (files on disk).
fn collect_symbols_from_disk(root: &Path, kinds: &[&str]) -> HashMap<String, ()> {
    let extractor = Extractor::new();
    let all_files = normalize_path_resolve::all_files(root, None);
    let mut map = HashMap::new();

    for entry in &all_files {
        if entry.kind != normalize_path_resolve::PathMatchKind::File {
            continue;
        }
        let abs_path = root.join(&entry.path);
        let Ok(content) = std::fs::read_to_string(&abs_path) else {
            continue;
        };

        let result = extractor.extract(&abs_path, &content);
        let rel = entry.path.replace('\\', "/");
        collect_recursive(&result.symbols, &rel, None, kinds, &mut map);
    }
    map
}

fn collect_recursive(
    symbols: &[normalize_facts::Symbol],
    rel_path: &str,
    parent_name: Option<&str>,
    kinds: &[&str],
    map: &mut HashMap<String, ()>,
) {
    for sym in symbols {
        let kind_str = sym.kind.as_str();
        if kinds.contains(&kind_str) {
            let key = if let Some(parent) = parent_name {
                format!("{rel_path}/{parent}/{}", sym.name)
            } else {
                format!("{rel_path}/{}", sym.name)
            };
            map.insert(key, ());
        }
        // Recurse into children (nested symbols like methods in classes)
        if !sym.children.is_empty() {
            collect_recursive(&sym.children, rel_path, Some(&sym.name), kinds, map);
        }
    }
}

/// Diff symbols of the given kinds between `base_ref` and the working tree.
pub(crate) fn symbol_diff(
    root: &Path,
    base_ref: &str,
    kinds: &[&str],
) -> anyhow::Result<Vec<DiffMeasurement>> {
    let base_map = collect_symbols_at_ref(root, base_ref, kinds)?;
    let current_map = collect_symbols_from_disk(root, kinds);

    let base_set: HashSet<&String> = base_map.keys().collect();
    let current_set: HashSet<&String> = current_map.keys().collect();

    let mut results = Vec::new();
    for key in current_set.difference(&base_set) {
        results.push(DiffMeasurement {
            key: (*key).clone(),
            added: 1.0,
            removed: 0.0,
        });
    }
    for key in base_set.difference(&current_set) {
        results.push(DiffMeasurement {
            key: (*key).clone(),
            added: 0.0,
            removed: 1.0,
        });
    }
    Ok(results)
}
