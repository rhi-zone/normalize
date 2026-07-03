//! Index acquisition: open, ensure-ready, and import-graph guards.
//!
//! These functions take the relevant config **slices** (`&IndexConfig`,
//! `&WalkConfig`) rather than the monolithic `NormalizeConfig`. This is what lets
//! feature crates acquire the index without depending on the main crate's config
//! type. The main crate's `crate::index` module provides thin wrappers that load
//! `NormalizeConfig` and pass `&config.index` / `&config.walk` in.

use crate::config::IndexConfig;
use normalize_facts::FileIndex;
use normalize_rules_config::WalkConfig;
use std::path::Path;

use crate::get_normalize_dir;

/// Open or create an index for a directory.
/// Index is stored in .normalize/index.sqlite (or NORMALIZE_INDEX_DIR if set).
pub async fn open(root: &Path, walk: &WalkConfig) -> Result<FileIndex, libsql::Error> {
    let moss_dir = get_normalize_dir(root);
    let db_path = moss_dir.join("index.sqlite");
    let mut idx = FileIndex::open(&db_path, root).await?;
    idx.set_walk_config(walk.clone().with_daemon_baseline());
    Ok(idx)
}

/// Open index only if indexing is enabled in config.
/// Returns None if `[index] enabled = false`.
pub async fn open_if_enabled(
    root: &Path,
    index: &IndexConfig,
    walk: &WalkConfig,
) -> Option<FileIndex> {
    if !index.enabled() {
        return None;
    }
    open(root, walk).await.ok()
}

/// Open index and ensure it has call graph data, auto-building if needed.
///
/// If the index is empty (no symbols), runs a full refresh + call graph build.
/// If the index exists but is stale, runs an incremental refresh.
/// Returns an error if indexing is disabled in config.
pub async fn ensure_ready(
    root: &Path,
    index: &IndexConfig,
    walk: &WalkConfig,
) -> Result<FileIndex, String> {
    if !index.enabled() {
        return Err(
            "Indexing is disabled. Enable it with: `normalize config set index.enabled true`"
                .to_string(),
        );
    }

    let mut idx = open(root, walk)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let stats = idx
        .call_graph_stats()
        .await
        .map_err(|e| format!("Failed to read index stats: {}", e))?;

    if stats.symbols == 0 {
        // No call graph data — full build needed
        eprintln!("Building facts index...");
        idx.refresh()
            .await
            .map_err(|e| format!("Failed to build file index: {}", e))?;
        let built = idx
            .refresh_call_graph()
            .await
            .map_err(|e| format!("Failed to build call graph: {}", e))?;
        eprintln!(
            "Indexed {} symbols, {} calls, {} imports",
            built.symbols, built.calls, built.imports
        );
    } else {
        // Index exists — incremental refresh if stale
        let file_changes = idx
            .incremental_refresh()
            .await
            .map_err(|e| format!("Incremental refresh failed: {}", e))?;
        if !file_changes.is_empty() {
            eprintln!("Refreshing index ({} files changed)...", file_changes.len());
            let updated = idx
                .incremental_call_graph_refresh()
                .await
                .map_err(|e| format!("Call graph refresh failed: {}", e))?;
            eprintln!(
                "Updated {} symbols, {} calls, {} imports",
                updated.symbols, updated.calls, updated.imports
            );
        }
    }

    Ok(idx)
}

/// Ensure the index is ready **and** contains import-graph data.
///
/// Like [`ensure_ready`], but additionally fails when the `imports` table is
/// empty — the state in which import-graph commands (`view graph`,
/// `view dependents`, `view import-path`, `rank imports`, `rank depth-map`,
/// `rank layering`, `analyze architecture`) cannot produce a meaningful answer.
///
/// This centralizes the distinction the CLI must make:
/// - **no import data at all** (`imports == 0`) → return `Err` so the command
///   exits non-zero with an actionable message instead of silently emitting a
///   zeroed report (a hard-constraint violation: "never silently return empty
///   results").
/// - **import data present, but this particular query is empty** (e.g.
///   `view import-path A B` with no path between them, or `view dependents X`
///   for an unimported module) → the guard passes and the command returns its
///   genuinely-empty result with exit 0.
///
/// The check is on the raw `imports` row count, not on post-filter results, so
/// legitimately-empty queries over a populated index are never misclassified.
pub async fn require_import_graph(
    root: &Path,
    index: &IndexConfig,
    walk: &WalkConfig,
) -> Result<FileIndex, String> {
    let idx = ensure_ready(root, index, walk).await?;
    let stats = idx
        .call_graph_stats()
        .await
        .map_err(|e| format!("Failed to read index stats: {}", e))?;
    if stats.imports == 0 {
        return Err(NO_IMPORT_DATA.to_string());
    }
    Ok(idx)
}

/// Actionable message returned by [`require_import_graph`] when the import graph
/// is empty. Names the rebuild command so both humans and agents know the fix.
pub const NO_IMPORT_DATA: &str = "No import data in the index — the import graph is empty. \
Run `normalize structure rebuild` to (re)build it. \
(If this codebase genuinely has no imports between its own files, that is expected.)";

/// Like [`ensure_ready`] but returns `Option` instead of `Result`.
///
/// When the index is unavailable (disabled, can't be opened, build fails),
/// prints a clear hint to stderr and returns `None` rather than failing the
/// whole command.  Use this for commands where the index *enriches* results
/// but isn't strictly required.
pub async fn ensure_ready_or_warn(
    root: &Path,
    index: &IndexConfig,
    walk: &WalkConfig,
) -> Option<FileIndex> {
    match ensure_ready(root, index, walk).await {
        Ok(idx) => Some(idx),
        Err(msg) => {
            eprintln!("hint: {msg}");
            None
        }
    }
}
