//! Facts management service for server-less CLI.

use crate::commands::facts::FactsContent;
use crate::index;
use crate::paths::get_normalize_dir;
use crate::skeleton;
use normalize_languages::external_packages;
use server_less::cli;
use std::cell::Cell;
use std::path::{Path, PathBuf};

/// Facts management sub-service.
pub struct FactsService {
    _pretty: Cell<bool>,
}

impl FactsService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            _pretty: Cell::new(pretty.get()),
        }
    }
}

/// Result of a rebuild operation.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RebuildResult {
    pub files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calls: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<usize>,
}

impl std::fmt::Display for RebuildResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Indexed {} files", self.files)?;
        let mut parts = Vec::new();
        if let Some(s) = self.symbols
            && s > 0
        {
            parts.push(format!("{} symbols", s));
        }
        if let Some(c) = self.calls
            && c > 0
        {
            parts.push(format!("{} calls", c));
        }
        if let Some(i) = self.imports
            && i > 0
        {
            parts.push(format!("{} imports", i));
        }
        if !parts.is_empty() {
            write!(f, "\nIndexed {}", parts.join(", "))?;
        }
        Ok(())
    }
}

/// Index statistics.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct FactsStats {
    pub db_size_bytes: u64,
    pub codebase_size_bytes: u64,
    pub ratio: f64,
    pub file_count: usize,
    pub dir_count: usize,
    pub symbol_count: usize,
    pub call_count: usize,
    pub import_count: usize,
    pub extensions: Vec<ExtensionCount>,
}

/// Extension count entry.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ExtensionCount {
    pub ext: String,
    pub count: usize,
}

impl std::fmt::Display for FactsStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Index Statistics")?;
        writeln!(f, "================")?;
        writeln!(f)?;
        writeln!(
            f,
            "Database:     {:.1} KB",
            self.db_size_bytes as f64 / 1024.0
        )?;
        writeln!(
            f,
            "Codebase:     {:.1} MB",
            self.codebase_size_bytes as f64 / 1024.0 / 1024.0
        )?;
        writeln!(f, "Ratio:        {:.2}%", self.ratio * 100.0)?;
        writeln!(f)?;
        writeln!(
            f,
            "Files:        {} ({} dirs)",
            self.file_count, self.dir_count
        )?;
        writeln!(f, "Symbols:      {}", self.symbol_count)?;
        writeln!(f, "Calls:        {}", self.call_count)?;
        writeln!(f, "Imports:      {}", self.import_count)?;
        writeln!(f)?;
        writeln!(f, "Top extensions:")?;
        for ec in self.extensions.iter().take(15) {
            writeln!(f, "  {:12} {:>6}", ec.ext, ec.count)?;
        }
        Ok(())
    }
}

/// Storage usage report.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StorageReport {
    pub index: StorageEntry,
    pub package_cache: StorageEntry,
    pub global_cache: StorageEntry,
    pub total_bytes: u64,
    pub total_human: String,
}

/// A single storage entry.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StorageEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub bytes: u64,
    pub human: String,
}

impl std::fmt::Display for StorageReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Storage Usage")?;
        writeln!(f)?;
        if let Some(ref p) = self.index.path {
            writeln!(f, "Project index:   {:>10}  {}", self.index.human, p)?;
        }
        if let Some(ref p) = self.package_cache.path {
            writeln!(
                f,
                "Package cache:   {:>10}  {}",
                self.package_cache.human, p
            )?;
        }
        if let Some(ref p) = self.global_cache.path {
            writeln!(f, "Global cache:    {:>10}  {}", self.global_cache.human, p)?;
        }
        writeln!(f)?;
        write!(f, "Total:           {:>10}", self.total_human)?;
        Ok(())
    }
}

/// File list result.
#[derive(serde::Serialize, schemars::JsonSchema)]
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

/// Package indexing result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PackagesResult {
    pub ecosystems: Vec<EcosystemCounts>,
}

/// Counts for a single ecosystem.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct EcosystemCounts {
    pub name: String,
    pub packages: usize,
    pub symbols: usize,
}

impl std::fmt::Display for PackagesResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Indexing complete:")?;
        for eco in &self.ecosystems {
            writeln!(
                f,
                "  {}: {} packages, {} symbols",
                eco.name, eco.packages, eco.symbols
            )?;
        }
        Ok(())
    }
}

/// Generic command result that wraps output from legacy functions.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else if self.success {
            write!(f, "Done")
        } else {
            write!(f, "Failed")
        }
    }
}

// =============================================================================
// Helper functions (inlined from commands/facts.rs service helpers)
// =============================================================================

fn get_cache_dir() -> Option<PathBuf> {
    if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
        Some(PathBuf::from(cache).join("normalize"))
    } else if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(".cache").join("normalize"))
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        Some(PathBuf::from(home).join(".cache").join("normalize"))
    } else {
        None
    }
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += dir_size(&p);
            }
        }
    }
    total
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buffer = [0u8; 8192];
    let Ok(bytes_read) = file.read(&mut buffer) else {
        return false;
    };
    buffer[..bytes_read].contains(&0)
}

fn build_storage_report(root: &Path) -> StorageReport {
    let index_path = root.join(".normalize").join("index.sqlite");
    let index_size = std::fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);

    let cache_dir = get_cache_dir().map(|d| d.join("packages"));
    let cache_size = cache_dir.as_ref().map(|d| dir_size(d)).unwrap_or(0);

    let global_cache_dir = get_cache_dir();
    let global_size = global_cache_dir.as_ref().map(|d| dir_size(d)).unwrap_or(0);

    StorageReport {
        index: StorageEntry {
            path: Some(index_path.display().to_string()),
            bytes: index_size,
            human: format_size(index_size),
        },
        package_cache: StorageEntry {
            path: cache_dir.as_ref().map(|d| d.display().to_string()),
            bytes: cache_size,
            human: format_size(cache_size),
        },
        global_cache: StorageEntry {
            path: global_cache_dir.as_ref().map(|d| d.display().to_string()),
            bytes: global_size,
            human: format_size(global_size),
        },
        total_bytes: index_size + global_size,
        total_human: format_size(index_size + global_size),
    }
}

/// Internal counts used during package indexing.
struct IndexedCounts {
    packages: usize,
    symbols: usize,
}

async fn count_and_insert_symbols(
    pkg_index: &external_packages::PackageIndex,
    pkg_id: i64,
    symbols: &[skeleton::SkeletonSymbol],
) -> usize {
    let mut count = 0;
    for sym in symbols {
        let _ = pkg_index
            .insert_symbol(
                pkg_id,
                &sym.name,
                sym.kind.as_str(),
                &sym.signature,
                sym.start_line as u32,
            )
            .await;
        count += 1;
        count += Box::pin(count_and_insert_symbols(pkg_index, pkg_id, &sym.children)).await;
    }
    count
}

async fn index_package_symbols(
    deps: &dyn normalize_local_deps::LocalDeps,
    pkg_index: &external_packages::PackageIndex,
    extractor: &mut skeleton::SkeletonExtractor,
    pkg_id: i64,
    path: &Path,
) -> usize {
    let entry = match deps.find_package_entry(path) {
        Some(e) => e,
        None => return 0,
    };
    if let Ok(content) = std::fs::read_to_string(&entry) {
        let result = extractor.extract(&entry, &content);
        return count_and_insert_symbols(pkg_index, pkg_id, &result.symbols).await;
    }
    0
}

async fn index_language_packages(
    deps: &dyn normalize_local_deps::LocalDeps,
    pkg_index: &external_packages::PackageIndex,
    project_root: &Path,
) -> IndexedCounts {
    let version = deps
        .get_version(project_root)
        .and_then(|v| external_packages::Version::parse(&v));

    let eco_key = deps.ecosystem_key();
    if eco_key.is_empty() {
        return IndexedCounts {
            packages: 0,
            symbols: 0,
        };
    }

    let sources = deps.dep_sources(project_root);
    if sources.is_empty() {
        return IndexedCounts {
            packages: 0,
            symbols: 0,
        };
    }

    let min_version = version.unwrap_or(external_packages::Version { major: 0, minor: 0 });
    let mut extractor = skeleton::SkeletonExtractor::new();
    let mut total_packages = 0;
    let mut total_symbols = 0;

    for source in sources {
        let max_version = if source.version_specific {
            version
        } else {
            None
        };
        let discovered = deps.discover_packages(&source);

        for (pkg_name, pkg_path) in discovered {
            if let Ok(true) = pkg_index.is_indexed(eco_key, &pkg_name).await {
                continue;
            }

            let pkg_id = match pkg_index
                .insert_package(
                    eco_key,
                    &pkg_name,
                    &pkg_path.to_string_lossy(),
                    min_version,
                    max_version,
                )
                .await
            {
                Ok(id) => id,
                Err(_) => continue,
            };

            total_packages += 1;
            total_symbols +=
                index_package_symbols(deps, pkg_index, &mut extractor, pkg_id, &pkg_path).await;
        }
    }

    IndexedCounts {
        packages: total_packages,
        symbols: total_symbols,
    }
}

// =============================================================================
// Async data functions
// =============================================================================

async fn rebuild_data(
    root: Option<&Path>,
    include: &[FactsContent],
) -> Result<RebuildResult, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let mut idx = index::open(&root)
        .await
        .map_err(|e| format!("Error opening index: {}", e))?;

    let file_count = idx
        .refresh()
        .await
        .map_err(|e| format!("Error refreshing index: {}", e))?;

    let mut result = RebuildResult {
        files: file_count,
        symbols: None,
        calls: None,
        imports: None,
    };

    if !include.is_empty() && !include.contains(&FactsContent::None) {
        let mut stats = idx
            .refresh_call_graph()
            .await
            .map_err(|e| format!("Error indexing call graph: {}", e))?;

        if !include.contains(&FactsContent::Symbols) {
            let _ = idx.execute("DELETE FROM symbols").await;
            stats.symbols = 0;
        }
        if !include.contains(&FactsContent::Calls) {
            let _ = idx.execute("DELETE FROM calls").await;
            stats.calls = 0;
        }
        if !include.contains(&FactsContent::Imports) {
            let _ = idx.execute("DELETE FROM imports").await;
            stats.imports = 0;
        }

        result.symbols = Some(stats.symbols);
        result.calls = Some(stats.calls);
        result.imports = Some(stats.imports);
    }

    Ok(result)
}

async fn stats_data(root: Option<&Path>) -> Result<FactsStats, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let moss_dir = get_normalize_dir(&root);
    let db_path = moss_dir.join("index.sqlite");
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let idx = index::open(&root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let files = idx
        .all_files()
        .await
        .map_err(|e| format!("Failed to read files: {}", e))?;

    let file_count = files.iter().filter(|f| !f.is_dir).count();
    let dir_count = files.iter().filter(|f| f.is_dir).count();

    let mut ext_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for f in &files {
        if f.is_dir {
            continue;
        }
        let path = std::path::Path::new(&f.path);
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_string(),
            None => {
                let full_path = root.join(&f.path);
                if is_binary_file(&full_path) {
                    "(binary)".to_string()
                } else {
                    "(no ext)".to_string()
                }
            }
        };
        *ext_counts.entry(ext).or_insert(0) += 1;
    }

    let mut ext_list: Vec<_> = ext_counts.into_iter().collect();
    ext_list.sort_by(|a, b| b.1.cmp(&a.1));

    let stats = idx.call_graph_stats().await.unwrap_or_default();

    let mut codebase_size = 0u64;
    for f in &files {
        if !f.is_dir {
            let full_path = root.join(&f.path);
            if let Ok(meta) = std::fs::metadata(&full_path) {
                codebase_size += meta.len();
            }
        }
    }

    Ok(FactsStats {
        db_size_bytes: db_size,
        codebase_size_bytes: codebase_size,
        ratio: if codebase_size > 0 {
            db_size as f64 / codebase_size as f64
        } else {
            0.0
        },
        file_count,
        dir_count,
        symbol_count: stats.symbols,
        call_count: stats.calls,
        import_count: stats.imports,
        extensions: ext_list
            .into_iter()
            .take(20)
            .map(|(ext, count)| ExtensionCount { ext, count })
            .collect(),
    })
}

async fn list_files_data(
    prefix: Option<&str>,
    root: Option<&Path>,
    limit: usize,
) -> Result<FileList, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let idx = index::open(&root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let files = idx
        .all_files()
        .await
        .map_err(|e| format!("Failed to read files: {}", e))?;

    let prefix_str = prefix.unwrap_or("");
    let filtered: Vec<String> = files
        .iter()
        .filter(|f| !f.is_dir && f.path.starts_with(prefix_str))
        .take(limit)
        .map(|f| f.path.clone())
        .collect();

    Ok(FileList { files: filtered })
}

async fn packages_data(
    only: &[String],
    clear: bool,
    root: Option<&Path>,
) -> Result<PackagesResult, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let pkg_index = external_packages::PackageIndex::open()
        .await
        .map_err(|e| format!("Failed to open package index: {}", e))?;

    if clear {
        pkg_index
            .clear()
            .await
            .map_err(|e| format!("Failed to clear index: {}", e))?;
        eprintln!("Cleared existing index");
    }

    let mut results: std::collections::HashMap<&str, IndexedCounts> =
        std::collections::HashMap::new();

    let all_deps = normalize_local_deps::registry::all_local_deps();
    let available: Vec<&str> = all_deps
        .iter()
        .map(|d| d.ecosystem_key())
        .filter(|k| !k.is_empty())
        .collect();

    let ecosystems: Vec<&str> = if only.is_empty() {
        available.clone()
    } else {
        only.iter()
            .map(|s| s.as_str())
            .filter(|s| available.contains(s))
            .collect()
    };

    for eco in only {
        if !available.contains(&eco.as_str()) {
            eprintln!(
                "Error: unknown ecosystem '{}', valid options: {}",
                eco,
                available.join(", ")
            );
        }
    }

    for deps in &all_deps {
        let eco_key = deps.ecosystem_key();
        if eco_key.is_empty() || !ecosystems.contains(&eco_key) {
            continue;
        }
        if results.contains_key(eco_key) {
            continue;
        }
        let counts = index_language_packages(*deps, &pkg_index, &root).await;
        results.insert(eco_key, counts);
    }

    Ok(PackagesResult {
        ecosystems: results
            .into_iter()
            .map(|(name, counts)| EcosystemCounts {
                name: name.to_string(),
                packages: counts.packages,
                symbols: counts.symbols,
            })
            .collect(),
    })
}

// =============================================================================
// Service impl
// =============================================================================

#[cli(
    name = "structure",
    about = "Manage the structural index (symbols, imports, calls)"
)]
impl FactsService {
    /// Rebuild the file index
    pub fn rebuild(
        &self,
        #[param(help = "What to extract: symbols, calls, imports (comma-separated)")] include: Vec<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RebuildResult, String> {
        let include: Vec<FactsContent> = if include.is_empty() {
            vec![
                FactsContent::Symbols,
                FactsContent::Calls,
                FactsContent::Imports,
            ]
        } else {
            include
                .iter()
                .map(|s| s.parse())
                .collect::<Result<Vec<_>, _>>()?
        };
        let root_path = root.map(PathBuf::from);
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(rebuild_data(root_path.as_deref(), &include))
    }

    /// Show index statistics (DB size vs codebase size)
    pub fn stats(
        &self,
        #[param(help = "Show storage usage for index and caches")] storage: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FactsStatsOutput, String> {
        if storage {
            let effective_root = root
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            return Ok(FactsStatsOutput::Storage(build_storage_report(
                &effective_root,
            )));
        }
        let root_path = root.map(PathBuf::from);
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(stats_data(root_path.as_deref()))
            .map(FactsStatsOutput::Stats)
    }

    /// List indexed files (with optional prefix filter)
    pub fn files(
        &self,
        #[param(positional, help = "Filter files by prefix")] prefix: Option<String>,
        #[param(short = 'l', help = "Maximum number of files to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FileList, String> {
        let limit = limit.unwrap_or(100);
        let root_path = root.map(PathBuf::from);
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(list_files_data(
            prefix.as_deref(),
            root_path.as_deref(),
            limit,
        ))
    }

    /// Index external packages (stdlib, site-packages) into global cache
    pub fn packages(
        &self,
        #[param(help = "Ecosystems to index (comma-separated)")] only: Vec<String>,
        #[param(help = "Clear existing index before re-indexing")] clear: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<PackagesResult, String> {
        let root_path = root.map(PathBuf::from);
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(packages_data(&only, clear, root_path.as_deref()))
    }
}

/// Output for stats command (either regular stats or storage report).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum FactsStatsOutput {
    Stats(FactsStats),
    Storage(StorageReport),
}

impl std::fmt::Display for FactsStatsOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stats(s) => write!(f, "{}", s),
            Self::Storage(s) => write!(f, "{}", s),
        }
    }
}
