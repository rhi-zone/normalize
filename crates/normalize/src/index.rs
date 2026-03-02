//! File indexing for fast code navigation.
//!
//! This module re-exports FileIndex from normalize_facts and provides
//! convenience wrappers that integrate with the CLI's config and paths.

use crate::config::NormalizeConfig;
use crate::paths::get_normalize_dir;
use std::path::Path;

// Re-export everything from normalize-facts index
pub use normalize_facts::{CallGraphStats, ChangedFiles, FileIndex, IndexedFile, SymbolMatch};

/// Open or create an index for a directory.
/// Index is stored in .normalize/index.sqlite (or NORMALIZE_INDEX_DIR if set)
pub async fn open(root: &Path) -> Result<FileIndex, libsql::Error> {
    let moss_dir = get_normalize_dir(root);
    let db_path = moss_dir.join("index.sqlite");
    FileIndex::open(&db_path, root).await
}

/// Open index only if indexing is enabled in config.
/// Returns None if `[index] enabled = false`.
pub async fn open_if_enabled(root: &Path) -> Option<FileIndex> {
    let config = NormalizeConfig::load(root);
    if !config.index.enabled() {
        return None;
    }
    open(root).await.ok()
}

/// Open index and ensure it has call graph data, auto-building if needed.
///
/// If the index is empty (no symbols), runs a full refresh + call graph build.
/// If the index exists but is stale, runs an incremental refresh.
/// Returns an error if indexing is disabled in config.
pub async fn ensure_ready(root: &Path) -> Result<FileIndex, String> {
    let config = NormalizeConfig::load(root);
    if !config.index.enabled() {
        return Err(
            "Indexing is disabled. Enable it with: `normalize config set index.enabled true`"
                .to_string(),
        );
    }

    let mut idx = open(root)
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
        if file_changes > 0 {
            eprintln!("Refreshing index ({} files changed)...", file_changes);
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
