//! Standalone CLI service for normalize-facts.
//!
//! Exposes `structure` subcommands: rebuild, stats, files.
//! This is a lightweight version of the `normalize structure` subcommands
//! that works without the full normalize binary.

use crate::FileIndex;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::{Path, PathBuf};

// =============================================================================
// Output types
// =============================================================================

/// Result of a rebuild operation.
#[derive(Serialize, JsonSchema)]
pub struct RebuildResult {
    pub files: usize,
}

impl std::fmt::Display for RebuildResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Indexed {} files", self.files)
    }
}

/// Index statistics.
#[derive(Serialize, JsonSchema)]
pub struct StatsResult {
    pub file_count: usize,
    pub dir_count: usize,
    pub symbol_count: usize,
    pub call_count: usize,
    pub import_count: usize,
    pub db_size_bytes: u64,
}

impl std::fmt::Display for StatsResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Files:    {} ({} dirs)", self.file_count, self.dir_count)?;
        writeln!(f, "Symbols:  {}", self.symbol_count)?;
        writeln!(f, "Calls:    {}", self.call_count)?;
        writeln!(f, "Imports:  {}", self.import_count)?;
        write!(f, "DB size:  {:.1} KB", self.db_size_bytes as f64 / 1024.0)
    }
}

/// File list result.
#[derive(Serialize, JsonSchema)]
pub struct FileList {
    pub files: Vec<String>,
}

impl std::fmt::Display for FileList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for path in &self.files {
            writeln!(f, "{}", path)?;
        }
        Ok(())
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn open_index_path(root: &Path) -> PathBuf {
    root.join(".normalize").join("index.sqlite")
}

async fn open_index(root: &Path) -> Result<FileIndex, String> {
    let db_path = open_index_path(root);
    FileIndex::open(&db_path, root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))
}

fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

// =============================================================================
// Service
// =============================================================================

/// Standalone CLI service for normalize-facts.
pub struct FactsCliService;

impl FactsCliService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FactsCliService {
    fn default() -> Self {
        Self::new()
    }
}

#[cli(
    name = "normalize-facts",
    version = "0.1.0",
    description = "Code fact extraction and index management"
)]
impl FactsCliService {
    /// Rebuild the file index (re-scan all files)
    pub async fn rebuild(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RebuildResult, String> {
        let root_path = resolve_root(root)?;
        let mut idx = open_index(&root_path).await?;
        let files = idx
            .refresh()
            .await
            .map_err(|e| format!("Error refreshing index: {}", e))?;
        Ok(RebuildResult { files })
    }

    /// Show index statistics
    pub async fn stats(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<StatsResult, String> {
        let root_path = resolve_root(root)?;
        let db_path = open_index_path(&root_path);
        let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

        let idx = open_index(&root_path).await?;
        let files = idx
            .all_files()
            .await
            .map_err(|e| format!("Failed to read files: {}", e))?;

        let file_count = files.iter().filter(|f| !f.is_dir).count();
        let dir_count = files.iter().filter(|f| f.is_dir).count();
        let graph_stats = idx.call_graph_stats().await.unwrap_or_default();

        Ok(StatsResult {
            file_count,
            dir_count,
            symbol_count: graph_stats.symbols,
            call_count: graph_stats.calls,
            import_count: graph_stats.imports,
            db_size_bytes: db_size,
        })
    }

    /// List indexed files (with optional prefix filter)
    pub async fn files(
        &self,
        #[param(positional, help = "Filter files by prefix")] prefix: Option<String>,
        #[param(short = 'l', help = "Maximum number of files to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FileList, String> {
        let root_path = resolve_root(root)?;
        let limit = limit.unwrap_or(100);
        let idx = open_index(&root_path).await?;
        let all = idx
            .all_files()
            .await
            .map_err(|e| format!("Failed to read files: {}", e))?;

        let prefix_str = prefix.as_deref().unwrap_or("");
        let filtered: Vec<String> = all
            .iter()
            .filter(|f| !f.is_dir && f.path.starts_with(prefix_str))
            .take(limit)
            .map(|f| f.path.clone())
            .collect();

        Ok(FileList { files: filtered })
    }
}
