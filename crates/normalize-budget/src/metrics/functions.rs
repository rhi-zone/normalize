//! Functions added/removed diff metric.

use super::{DiffMeasurement, DiffMetric};
use normalize_facts::Extractor;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

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

/// Create a temporary git worktree at the given ref. Returns the worktree path.
pub(crate) fn create_worktree(root: &Path, base_ref: &str) -> anyhow::Result<std::path::PathBuf> {
    let hash_output = Command::new("git")
        .args(["rev-parse", "--verify", base_ref])
        .current_dir(root)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run git: {e}"))?;

    if !hash_output.status.success() {
        return Err(anyhow::anyhow!(
            "git ref '{base_ref}' not found: {}",
            String::from_utf8_lossy(&hash_output.stderr).trim()
        ));
    }

    let hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();
    // SAFETY: git hashes are always ASCII hex digits, so byte indexing is char-boundary-safe.
    let short = &hash[..7.min(hash.len())];
    // Include PID to avoid race conditions when multiple normalize processes run concurrently.
    let worktree_name = format!("normalize-budget-wt-{}-{}", short, std::process::id());
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    // Clean up any stale worktree (same PID, same hash — should not normally exist)
    if worktree_path.exists() {
        let rm_out = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
        match rm_out {
            Ok(o) if !o.status.success() => {
                tracing::warn!(
                    "git worktree remove failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                )
            }
            Err(e) => tracing::warn!("git worktree remove error: {e}"),
            _ => {}
        }
    }

    let add_output = Command::new("git")
        .args(["worktree", "add", "--detach", &worktree_str, &hash])
        .current_dir(root)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to create worktree: {e}"))?;

    if !add_output.status.success() {
        return Err(anyhow::anyhow!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_output.stderr).trim()
        ));
    }

    Ok(worktree_path)
}

/// RAII guard that removes a git worktree on drop.
pub(crate) struct WorktreeGuard {
    pub(crate) path: std::path::PathBuf,
    pub(crate) root: std::path::PathBuf,
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        let worktree_str = self.path.to_string_lossy().to_string();
        let _ = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(&self.root)
            .output();
    }
}

/// Remove a temporary git worktree. Propagates errors on failure.
#[allow(dead_code)]
pub(crate) fn remove_worktree(root: &Path, worktree_path: &Path) -> anyhow::Result<()> {
    let worktree_str = worktree_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run git worktree remove: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn collect_symbols_for_kinds(scan_root: &Path, kinds: &[&str]) -> HashMap<String, ()> {
    let extractor = Extractor::new();
    let all_files = normalize_path_resolve::all_files(scan_root, None);
    let mut map = HashMap::new();

    for entry in &all_files {
        if entry.kind != normalize_path_resolve::PathMatchKind::File {
            continue;
        }
        let abs_path = scan_root.join(&entry.path);
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
    let worktree_path = create_worktree(root, base_ref)?;
    let _guard = WorktreeGuard {
        path: worktree_path.clone(),
        root: root.to_path_buf(),
    };
    let base_map = collect_symbols_for_kinds(&worktree_path, kinds);
    drop(_guard);

    let current_map = collect_symbols_for_kinds(root, kinds);

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
