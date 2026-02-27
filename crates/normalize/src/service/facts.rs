//! Facts management service for server-less CLI.

use crate::commands::facts::FactsContent;
use server_less::cli;
use std::cell::Cell;

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

#[cli(
    name = "facts",
    about = "Manage code facts (file index, symbols, calls, imports)"
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
        crate::commands::facts::cmd_rebuild_service(root.as_deref(), &include)
    }

    /// Show index statistics (DB size vs codebase size)
    pub fn stats(
        &self,
        #[param(help = "Show storage usage for index and caches")] storage: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FactsStatsOutput, String> {
        crate::commands::facts::cmd_stats_service(root.as_deref(), storage)
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
        crate::commands::facts::cmd_list_files_service(prefix.as_deref(), root.as_deref(), limit)
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
        crate::commands::facts::cmd_packages_service(&only, clear, root.as_deref())
    }

    /// Run compiled rule packs (dylibs) against extracted facts
    pub fn rules(
        &self,
        #[param(help = "Specific rule to run (runs all if not specified)")] rule: Option<String>,
        #[param(help = "Path to a specific rule pack dylib")] pack: Option<String>,
        #[param(help = "List available rules instead of running them")] list: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<CommandResult, String> {
        crate::commands::facts::cmd_facts_rules_service(
            root.as_deref(),
            rule.as_deref(),
            pack.as_deref(),
            list,
        )
    }

    /// Run Datalog rules (.dl) against extracted facts
    pub fn check(
        &self,
        #[param(positional, help = "Path to a specific .dl rules file")] rules_file: Option<String>,
        #[param(help = "List available rules instead of running them")] list: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<CommandResult, String> {
        crate::commands::facts::cmd_check_service(root.as_deref(), rules_file.as_deref(), list)
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
