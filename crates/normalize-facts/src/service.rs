//! Standalone CLI service for normalize-facts.
//!
//! Exposes `structure` subcommands: rebuild, stats, files.
//! This is a lightweight version of the `normalize structure` subcommands
//! that works without the full normalize binary.

use crate::FileIndex;
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::{Path, PathBuf};

// =============================================================================
// Output types
// =============================================================================

/// Report for a rebuild operation.
#[derive(Serialize, JsonSchema)]
pub struct RebuildReport {
    pub files: usize,
}

impl OutputFormatter for RebuildReport {
    fn format_text(&self) -> String {
        format!("Indexed {} files", self.files)
    }
}

/// Index statistics.
#[derive(Serialize, JsonSchema)]
pub struct StructureStatsReport {
    pub file_count: usize,
    pub dir_count: usize,
    pub symbol_count: usize,
    pub call_count: usize,
    pub import_count: usize,
    pub db_size_bytes: u64,
}

impl OutputFormatter for StructureStatsReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Files:    {} ({} dirs)\n",
            self.file_count, self.dir_count
        ));
        out.push_str(&format!("Symbols:  {}\n", self.symbol_count));
        out.push_str(&format!("Calls:    {}\n", self.call_count));
        out.push_str(&format!("Imports:  {}\n", self.import_count));
        out.push_str(&format!(
            "DB size:  {:.1} KB",
            self.db_size_bytes as f64 / 1024.0
        ));
        out
    }
}

/// File list result.
#[derive(Serialize, JsonSchema)]
pub struct StructureFilesReport {
    pub files: Vec<String>,
}

impl OutputFormatter for StructureFilesReport {
    fn format_text(&self) -> String {
        self.files.iter().map(|p| format!("{}\n", p)).collect()
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

impl FactsCliService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(
    name = "normalize-facts",
    version = "0.1.0",
    description = "Code fact extraction and index management"
)]
impl FactsCliService {
    /// Rebuild the file index (re-scan all files)
    #[cli(display_with = "display_output")]
    pub async fn rebuild(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RebuildReport, String> {
        let root_path = resolve_root(root)?;
        let mut idx = open_index(&root_path).await?;
        let files = idx
            .refresh()
            .await
            .map_err(|e| format!("Error refreshing index: {}", e))?;
        Ok(RebuildReport { files })
    }

    /// Show index statistics
    #[cli(display_with = "display_output")]
    pub async fn stats(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<StructureStatsReport, String> {
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

        Ok(StructureStatsReport {
            file_count,
            dir_count,
            symbol_count: graph_stats.symbols,
            call_count: graph_stats.calls,
            import_count: graph_stats.imports,
            db_size_bytes: db_size,
        })
    }

    /// List indexed files (with optional prefix filter)
    #[cli(display_with = "display_output")]
    pub async fn files(
        &self,
        #[param(positional, help = "Filter files by prefix")] prefix: Option<String>,
        #[param(short = 'l', help = "Maximum number of files to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<StructureFilesReport, String> {
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

        Ok(StructureFilesReport { files: filtered })
    }
}
