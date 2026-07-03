//! File indexing for fast code navigation.
//!
//! The real acquisition logic lives in the `normalize-index` leaf crate, which
//! takes config **slices** (`&IndexConfig`, `&WalkConfig`) so feature crates can
//! acquire the index without depending on `NormalizeConfig`. This module provides
//! thin wrappers that load `NormalizeConfig` and pass the relevant slices in, so
//! the CLI's existing `root`-only call sites are unchanged.

use crate::config::NormalizeConfig;
use std::path::Path;

// Re-export the index types and import-graph API from normalize-index (which
// re-exports the FileIndex family from normalize-facts).
pub use normalize_index::{
    CallGraphStats, ChangedFiles, FileIndex, ImportGraph, IndexedFile, NO_IMPORT_DATA, SymbolMatch,
    build_import_graph,
};

/// Open or create an index for a directory.
/// Index is stored in .normalize/index.sqlite (or NORMALIZE_INDEX_DIR if set).
pub async fn open(root: &Path) -> Result<FileIndex, libsql::Error> {
    let config = NormalizeConfig::load(root);
    normalize_index::open(root, &config.walk).await
}

/// Open index only if indexing is enabled in config.
/// Returns None if `[index] enabled = false`.
pub async fn open_if_enabled(root: &Path) -> Option<FileIndex> {
    let config = NormalizeConfig::load(root);
    normalize_index::open_if_enabled(root, &config.index, &config.walk).await
}

/// Open index and ensure it has call graph data, auto-building if needed.
///
/// Returns an error if indexing is disabled in config.
pub async fn ensure_ready(root: &Path) -> Result<FileIndex, String> {
    let config = NormalizeConfig::load(root);
    normalize_index::ensure_ready(root, &config.index, &config.walk).await
}

/// Ensure the index is ready **and** contains import-graph data.
pub async fn require_import_graph(root: &Path) -> Result<FileIndex, String> {
    let config = NormalizeConfig::load(root);
    normalize_index::require_import_graph(root, &config.index, &config.walk).await
}

/// Like [`ensure_ready`] but returns `Option`, printing a hint to stderr on failure.
pub async fn ensure_ready_or_warn(root: &Path) -> Option<FileIndex> {
    let config = NormalizeConfig::load(root);
    normalize_index::ensure_ready_or_warn(root, &config.index, &config.walk).await
}
