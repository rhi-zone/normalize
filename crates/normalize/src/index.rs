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
