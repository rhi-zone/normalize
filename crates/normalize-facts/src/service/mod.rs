//! Standalone CLI service for normalize-facts — the canonical `structure` verb.
//!
//! Exposes `structure` subcommands: rebuild, stats, files, packages, query,
//! test-fixtures, and the CFG dataflow trio (liveness, effects, exceptions).
//!
//! The service owns its config access: it loads the `[walk]` and `[aliases]`
//! sections directly from the global and project `config.toml` files (the
//! "sessions technique"), so this crate does not depend on the main crate's
//! monolithic `NormalizeConfig`. It reads and writes the SQLite index directly
//! via [`FileIndex`] (this crate *is* the index crate).

pub mod effects;
pub mod exceptions;
pub mod liveness;

pub use effects::{EffectEntry, EffectsReport, FunctionEffects, analyze_effects};
pub use exceptions::{
    CatchEntry, ExceptionsReport, FunctionExceptions, ThrowEntry, analyze_exceptions,
};
pub use liveness::{BlockLiveness, LivenessReport, analyze_liveness};

use crate::FileIndex;
use crate::paths::get_normalize_dir;
use normalize_filter::{AliasConfig, Filter};
use normalize_languages::external_packages;
use normalize_output::OutputFormatter;
use normalize_rules_config::WalkConfig;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::{Path, PathBuf};

fn is_false(b: &bool) -> bool {
    !b
}

// =============================================================================
// Content selector (local copy — the main crate has its own for other uses)
// =============================================================================

/// What to extract during indexing (files are always indexed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FactsContent {
    /// Skip content extraction (files only)
    None,
    /// Function and type definitions
    Symbols,
    /// Function call relationships
    Calls,
    /// Import statements
    Imports,
}

impl std::str::FromStr for FactsContent {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "symbols" => Ok(Self::Symbols),
            "calls" => Ok(Self::Calls),
            "imports" => Ok(Self::Imports),
            _ => Err(format!("unknown facts content: {s}")),
        }
    }
}

// =============================================================================
// Config / index acquisition helpers (tolerant slice loaders — no NormalizeConfig)
// =============================================================================

fn open_index_path(root: &Path) -> PathBuf {
    // Honor NORMALIZE_INDEX_DIR / XDG resolution so `structure` and the main
    // binary's cross-file commands agree on where the index lives.
    get_normalize_dir(root).join("index.sqlite")
}

/// Parse `[walk]` from the global then project `config.toml`, always applying the
/// daemon baseline (`.git/`, `.normalize/`) so the index walkers never descend
/// into `.normalize/` even when no `[walk]` section is present. Delegates to the
/// shared [`normalize_config_paths::ConfigSlices::walk`] loader.
fn load_walk_config(root: &Path) -> WalkConfig {
    normalize_config_paths::ConfigSlices::load(root).walk()
}

async fn open_index(root: &Path) -> Result<FileIndex, String> {
    let db_path = open_index_path(root);
    let mut idx = FileIndex::open(&db_path, root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;
    idx.set_walk_config(load_walk_config(root));
    Ok(idx)
}

/// Open the index and ensure it has call-graph (and CFG) data, building it if
/// absent and incrementally refreshing it when stale. Mirrors the main crate's
/// `index::ensure_ready` but without the `[index] enabled` gate (a main-crate
/// config concept).
async fn ensure_ready(root: &Path) -> Result<FileIndex, String> {
    let mut idx = open_index(root).await?;
    let stats = idx
        .call_graph_stats()
        .await
        .map_err(|e| format!("Failed to read index stats: {}", e))?;
    if stats.symbols == 0 {
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

fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

/// Resolve a caller-supplied file path to a root-relative path for index queries.
fn rel_to_root(root: &Path, file: &str) -> String {
    let abs_file = Path::new(file);
    if abs_file.is_absolute() {
        abs_file
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| file.to_string())
    } else {
        file.to_string()
    }
}

// =============================================================================
// Filter (aliases loaded standalone from config.toml — no NormalizeConfig)
// =============================================================================

/// Load just the `[aliases]` slice from the global then project `config.toml` via
/// the shared [`normalize_config_paths::ConfigSlices`] loader.
fn load_aliases(root: &Path) -> AliasConfig {
    normalize_config_paths::ConfigSlices::load(root).slice("aliases")
}

/// Detect programming languages present under `root` (bounded-depth walk).
/// Used to resolve language-scoped filter aliases (`@tests`, …).
fn detect_project_languages(root: &Path) -> Vec<String> {
    let mut languages = std::collections::HashSet::new();
    let walker = ignore::WalkBuilder::new(root)
        .max_depth(Some(5))
        .hidden(false)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if let Some(lang) = normalize_languages::support_for_path(entry.path()) {
            languages.insert(lang.name().to_string());
        }
    }
    let mut result: Vec<_> = languages.into_iter().collect();
    result.sort();
    result
}

/// Build a `Filter` from `--exclude` / `--only` patterns, printing any warnings.
/// Returns `None` when both slices are empty (no filtering needed).
fn build_filter(root: &Path, exclude: &[String], only: &[String]) -> Option<Filter> {
    if exclude.is_empty() && only.is_empty() {
        return None;
    }
    let aliases = load_aliases(root);
    let languages = detect_project_languages(root);
    let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();
    match Filter::new(exclude, only, &aliases, &lang_refs) {
        Ok(f) => {
            for warning in f.warnings() {
                eprintln!("warning: {}", warning);
            }
            Some(f)
        }
        Err(e) => {
            eprintln!("error: {}", e);
            None
        }
    }
}

// =============================================================================
// Output types
// =============================================================================

/// One missing-grammar entry on a `RebuildReport`.
#[derive(Serialize, JsonSchema)]
pub struct MissingGrammarEntry {
    /// Grammar name (e.g. `"go"`, `"kotlin"`).
    pub grammar: String,
    /// Number of files that requested this grammar and were skipped.
    pub files: usize,
    /// Human-readable error detail.
    pub detail: String,
}

/// Report for `normalize structure rebuild`: counts of indexed entities.
///
/// `files` is always populated. `symbols`, `calls`, and `imports` are only set when the
/// corresponding content type was included in the rebuild (controlled by `--include`).
#[derive(Serialize, JsonSchema)]
pub struct RebuildReport {
    pub files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calls: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<usize>,
    /// Number of co-change edge pairs written during this rebuild.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub co_change_edges: Option<usize>,
    /// Whether this was an incremental rebuild (true) or a full rebuild (false).
    #[serde(skip_serializing_if = "is_false")]
    pub incremental: bool,
    /// Grammars that failed to load during this rebuild (one entry per grammar).
    /// Files needing those grammars were skipped, leaving the index incomplete.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub missing_grammars: Vec<MissingGrammarEntry>,
    /// True if this was a dry-run preview (the index was not opened or written).
    #[serde(skip_serializing_if = "is_false", default)]
    pub dry_run: bool,
    /// In dry-run mode, a human-readable description of what would be rebuilt.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub plan: Option<String>,
}

impl OutputFormatter for RebuildReport {
    fn format_text(&self) -> String {
        if self.dry_run {
            return self
                .plan
                .clone()
                .unwrap_or_else(|| "[dry-run] Would rebuild the index.".to_string());
        }
        if self.incremental && self.files == 0 {
            return "Index up to date".to_string();
        }
        let mode = if self.incremental {
            " (incremental)"
        } else {
            ""
        };
        let mut out = format!("Indexed {} files{}", self.files, mode);
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
        if let Some(e) = self.co_change_edges
            && e > 0
        {
            parts.push(format!("{} co-change edges", e));
        }
        if !parts.is_empty() {
            out.push_str(&format!("\nIndexed {}", parts.join(", ")));
        }
        if !self.missing_grammars.is_empty() {
            let total: usize = self.missing_grammars.iter().map(|m| m.files).sum();
            let breakdown = self
                .missing_grammars
                .iter()
                .map(|m| {
                    let noun = if m.files == 1 { "file" } else { "files" };
                    format!("{} ({} {})", m.grammar, m.files, noun)
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "\nSkipped {total} files due to missing grammars: {breakdown}\nRun `normalize grammars install` to extract all languages."
            ));
        }
        out
    }
}

/// Index statistics returned by `normalize structure stats`.
///
/// Includes database size, codebase size, ratio, entity counts (files, symbols, calls,
/// imports), and a ranked list of file extensions present in the index.
#[derive(Serialize, JsonSchema)]
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

/// Count of indexed files with a given file extension.
#[derive(Serialize, JsonSchema)]
pub struct ExtensionCount {
    pub ext: String,
    pub count: usize,
}

impl OutputFormatter for FactsStats {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Index Statistics");
        let _ = writeln!(out, "================");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Database:     {:.1} KB",
            self.db_size_bytes as f64 / 1024.0
        );
        let _ = writeln!(
            out,
            "Codebase:     {:.1} MB",
            self.codebase_size_bytes as f64 / 1024.0 / 1024.0
        );
        let _ = writeln!(out, "Ratio:        {:.2}%", self.ratio * 100.0);
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Files:        {} ({} dirs)",
            self.file_count, self.dir_count
        );
        let _ = writeln!(out, "Symbols:      {}", self.symbol_count);
        let _ = writeln!(out, "Calls:        {}", self.call_count);
        let _ = writeln!(out, "Imports:      {}", self.import_count);
        let _ = writeln!(out);
        let _ = writeln!(out, "Top extensions:");
        for ec in self.extensions.iter().take(15) {
            let _ = writeln!(out, "  {:12} {:>6}", ec.ext, ec.count);
        }
        out
    }
}

impl std::fmt::Display for FactsStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Storage usage report returned by `normalize structure stats --storage`.
///
/// Breaks down disk usage into the project index, language package cache, and global cache.
#[derive(Serialize, JsonSchema)]
pub struct StorageReport {
    pub index: StorageEntry,
    pub package_cache: StorageEntry,
    pub global_cache: StorageEntry,
    pub total_bytes: u64,
    pub total_human: String,
}

/// A single storage location's disk usage, with optional path and human-readable size.
#[derive(Serialize, JsonSchema)]
pub struct StorageEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub bytes: u64,
    pub human: String,
}

impl OutputFormatter for StorageReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Storage Usage");
        let _ = writeln!(out);
        if let Some(ref p) = self.index.path {
            let _ = writeln!(out, "Project index:   {:>10}  {}", self.index.human, p);
        }
        if let Some(ref p) = self.package_cache.path {
            let _ = writeln!(
                out,
                "Package cache:   {:>10}  {}",
                self.package_cache.human, p
            );
        }
        if let Some(ref p) = self.global_cache.path {
            let _ = writeln!(
                out,
                "Global cache:    {:>10}  {}",
                self.global_cache.human, p
            );
        }
        let _ = writeln!(out);
        let _ = write!(out, "Total:           {:>10}", self.total_human);
        out
    }
}

/// List of indexed file paths returned by `normalize structure files`.
///
/// Each entry is a path relative to the project root. The list can be filtered by prefix
/// and capped via `--limit`.
#[derive(Serialize, JsonSchema)]
pub struct FileListReport {
    pub files: Vec<String>,
}

impl OutputFormatter for FileListReport {
    fn format_text(&self) -> String {
        self.files.iter().map(|p| format!("{}\n", p)).collect()
    }
}

/// Report for `normalize structure packages`: indexed package counts per ecosystem.
#[derive(Serialize, JsonSchema)]
pub struct PackagesReport {
    pub ecosystems: Vec<EcosystemCounts>,
}

/// Package and symbol counts for a single package ecosystem (e.g. "rust", "python").
#[derive(Serialize, JsonSchema)]
pub struct EcosystemCounts {
    pub name: String,
    pub packages: usize,
    pub symbols: usize,
}

impl OutputFormatter for PackagesReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        // Never return an empty string: an agent or user must always see a clear
        // outcome. When no ecosystems were indexed, say so explicitly rather than
        // printing a bare "Indexing complete:" with no rows (or nothing at all).
        if self.ecosystems.is_empty() {
            return "No package ecosystems detected to index. \
Add dependencies (e.g. a Cargo.toml or requirements.txt) or pass --only <ecosystem>."
                .to_string();
        }
        let mut out = String::new();
        let _ = writeln!(out, "Indexing complete:");
        for eco in &self.ecosystems {
            let _ = writeln!(
                out,
                "  {}: {} packages, {} symbols",
                eco.name, eco.packages, eco.symbols
            );
        }
        out
    }
}

/// Report for stats command (either regular stats or storage report).
#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum FactsStatsReport {
    Stats(FactsStats),
    Storage(StorageReport),
}

impl OutputFormatter for FactsStatsReport {
    fn format_text(&self) -> String {
        match self {
            Self::Stats(s) => s.format_text(),
            Self::Storage(s) => s.format_text(),
        }
    }
}

/// Report for a raw SQL query against the structural index.
#[derive(Serialize, JsonSchema)]
pub struct QueryReport {
    pub rows: Vec<serde_json::Map<String, serde_json::Value>>,
}

impl OutputFormatter for QueryReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        if self.rows.is_empty() {
            return "(no rows)".to_string();
        }
        let mut out = String::new();
        let cols: Vec<&str> = self.rows[0].keys().map(|k| k.as_str()).collect();
        let mut widths: Vec<usize> = cols.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, col) in cols.iter().enumerate() {
                let val = row.get(*col).map(value_to_str).unwrap_or_default();
                if val.len() > widths[i] {
                    widths[i] = val.len();
                }
            }
        }
        let header: Vec<String> = cols
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{:width$}", c, width = w))
            .collect();
        let _ = writeln!(out, "{}", header.join("  "));
        let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        let _ = writeln!(out, "{}", sep.join("  "));
        for row in &self.rows {
            let cells: Vec<String> = cols
                .iter()
                .zip(&widths)
                .map(|(col, w)| {
                    let val = row.get(*col).map(value_to_str).unwrap_or_default();
                    format!("{:width$}", val, width = w)
                })
                .collect();
            let _ = writeln!(out, "{}", cells.join("  "));
        }
        let _ = write!(out, "\n{} row(s)", self.rows.len());
        out
    }
}

fn value_to_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Report returned by `normalize structure test-fixtures`.
#[derive(Serialize, JsonSchema)]
pub struct ExtractionFixtureTestReport {
    pub fixture_dir: String,
    pub cases: Vec<ExtractionFixtureCaseResult>,
    pub passed: usize,
    pub failed: usize,
    pub updated: bool,
}

impl OutputFormatter for ExtractionFixtureTestReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for case in &self.cases {
            if case.passed {
                let _ = writeln!(out, "PASS  {}", case.case);
            } else {
                let _ = writeln!(out, "FAIL  {}", case.case);
                for line in &case.diff {
                    let _ = writeln!(out, "      {line}");
                }
            }
        }
        if self.updated {
            let _ = write!(out, "\nUpdated {} fixture case(s).", self.cases.len());
        } else {
            let _ = write!(out, "\n{} passed, {} failed", self.passed, self.failed);
        }
        out
    }
}

/// Result for a single extraction fixture case.
#[derive(Serialize, JsonSchema)]
pub struct ExtractionFixtureCaseResult {
    pub case: String,
    pub passed: bool,
    pub diff: Vec<String>,
}

// =============================================================================
// Package-indexing helpers (inlined from the main crate's facts service)
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
    symbols: &[crate::Symbol],
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
    extractor: &crate::extract::Extractor,
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
    let extractor = crate::extract::Extractor::new();
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
                index_package_symbols(deps, pkg_index, &extractor, pkg_id, &pkg_path).await;
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

#[allow(clippy::too_many_arguments)]
async fn rebuild_data(
    root: Option<&Path>,
    include: &[FactsContent],
    only: &[String],
    exclude: &[String],
    full: bool,
    strict: bool,
    dry_run: bool,
) -> Result<RebuildReport, String> {
    // Reset the missing-grammar tracker so this rebuild starts fresh — any
    // missing grammars surfaced below belong to *this* rebuild only.
    let _ = crate::take_missing_grammars();
    let root = root
        .map(|p| p.to_path_buf())
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {e}"))?;

    let filter = build_filter(&root, exclude, only);

    if dry_run {
        let mode = if full {
            "full rebuild"
        } else {
            "incremental rebuild"
        };
        let content: Vec<&str> = include
            .iter()
            .filter(|c| **c != FactsContent::None)
            .map(|c| match c {
                FactsContent::Symbols => "symbols",
                FactsContent::Calls => "calls",
                FactsContent::Imports => "imports",
                FactsContent::None => "",
            })
            .collect();
        let content_str = if content.is_empty() {
            "file tree only".to_string()
        } else {
            content.join(", ")
        };
        let index_path = root.join(".normalize").join("index.sqlite");
        let mut plan = format!(
            "[dry-run] Would perform a {mode} of the index at {}.\n[dry-run] Root: {}\n[dry-run] Content: {content_str}",
            index_path.display(),
            root.display(),
        );
        if !only.is_empty() {
            plan.push_str(&format!("\n[dry-run] Only: {}", only.join(", ")));
        }
        if !exclude.is_empty() {
            plan.push_str(&format!("\n[dry-run] Exclude: {}", exclude.join(", ")));
        }
        plan.push_str("\n[dry-run] No files were parsed and the index was not modified.");
        return Ok(RebuildReport {
            files: 0,
            symbols: None,
            calls: None,
            imports: None,
            co_change_edges: None,
            incremental: !full,
            missing_grammars: Vec::new(),
            dry_run: true,
            plan: Some(plan),
        });
    }

    let mut idx = open_index(&root).await?;

    idx.set_progress(true);

    let file_count = if full {
        idx.refresh()
            .await
            .map_err(|e| format!("Error refreshing index: {}", e))?
    } else {
        idx.incremental_refresh()
            .await
            .map_err(|e| format!("Error refreshing index: {}", e))?
            .len()
    };

    // If a filter is active, remove indexed files that don't match it.
    // We do this after the full walk so the index's file-tree relationships
    // are consistent, and then prune what the caller asked to exclude.
    let file_count = if let Some(ref f) = filter {
        let all_paths: Vec<String> = idx
            .all_files()
            .await
            .map_err(|e| format!("Error listing indexed files: {}", e))?
            .into_iter()
            .filter(|entry| {
                let path = Path::new(&entry.path);
                !f.matches(path)
            })
            .map(|entry| entry.path)
            .collect();
        for path in &all_paths {
            let _ = idx
                .execute(&format!(
                    "DELETE FROM files WHERE path = '{}'",
                    path.replace('\'', "''")
                ))
                .await;
        }
        file_count.saturating_sub(all_paths.len())
    } else {
        file_count
    };

    let mut result = RebuildReport {
        files: file_count,
        symbols: None,
        calls: None,
        imports: None,
        co_change_edges: None,
        incremental: !full,
        missing_grammars: Vec::new(),
        dry_run: false,
        plan: None,
    };

    if !include.is_empty() && !include.contains(&FactsContent::None) {
        let mut stats = if full {
            idx.refresh_call_graph()
                .await
                .map_err(|e| format!("Error indexing call graph: {}", e))?
        } else {
            idx.incremental_call_graph_refresh()
                .await
                .map_err(|e| format!("Error indexing call graph: {}", e))?
        };

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

    // Populate co-change edges from git history.
    // For incremental rebuilds, pass the last-recorded HEAD SHA so we only process
    // new commits. For full rebuilds, pass None to rebuild from scratch.
    let since = if !full {
        idx.co_change_last_commit().await
    } else {
        None
    };
    match idx.rebuild_co_change_edges(since.as_deref()).await {
        Ok(edge_count) => {
            if edge_count > 0 || full {
                result.co_change_edges = Some(edge_count);
            }
        }
        Err(e) => {
            tracing::warn!("co-change edge rebuild failed (non-fatal): {}", e);
        }
    }

    // Drain missing-grammar tracker into the report so users see (and can
    // act on) which languages were silently skipped.
    let mut missing: Vec<MissingGrammarEntry> = crate::take_missing_grammars()
        .into_iter()
        .map(|m| MissingGrammarEntry {
            grammar: m.name,
            files: m.count,
            detail: m.detail,
        })
        .collect();
    missing.sort_by(|a, b| {
        b.files
            .cmp(&a.files)
            .then_with(|| a.grammar.cmp(&b.grammar))
    });

    if strict && !missing.is_empty() {
        let breakdown = missing
            .iter()
            .map(|m| {
                let noun = if m.files == 1 { "file" } else { "files" };
                format!("{} ({} {})", m.grammar, m.files, noun)
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "missing grammars (strict mode): {breakdown}. Run `normalize grammars install`."
        ));
    }

    result.missing_grammars = missing;

    Ok(result)
}

async fn stats_data(root: Option<&Path>) -> Result<FactsStats, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {e}"))?;

    let moss_dir = get_normalize_dir(&root);
    let db_path = moss_dir.join("index.sqlite");
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let idx = open_index(&root).await?;

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
        let path = Path::new(&f.path);
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
    ext_list.sort_by_key(|b| std::cmp::Reverse(b.1));

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
) -> Result<FileListReport, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {e}"))?;

    let idx = open_index(&root).await?;

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

    Ok(FileListReport { files: filtered })
}

async fn packages_data(
    only: &[String],
    clear: bool,
    root: Option<&Path>,
) -> Result<PackagesReport, String> {
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
        tracing::info!("Cleared existing index");
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
            tracing::warn!(
                "unknown ecosystem '{}', valid options: {}",
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

    Ok(PackagesReport {
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
// Service
// =============================================================================

/// Canonical CLI service backing the `structure` verb.
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
    name = "structure",
    version = "0.3.2",
    description = "Build and query the code index. Run `structure rebuild` after cloning or when cross-file commands return stale results."
)]
impl FactsCliService {
    /// Rebuild the structural index (symbols, calls, imports, and file tree)
    ///
    /// Walks the project directory, parses source files, and populates the SQLite index
    /// at `.normalize/index.sqlite`. Required before running fact rules or cross-file
    /// navigation commands (referenced-by, dependents, depth-map, etc.).
    ///
    /// Examples:
    ///   normalize structure rebuild                              # rebuild with all content types
    ///   normalize structure rebuild --include symbols            # only extract symbols
    ///   normalize structure rebuild --include calls,imports      # extract calls and imports
    ///   normalize structure rebuild --only "src/**"              # only index files under src/
    ///   normalize structure rebuild --exclude "vendor/**"        # skip vendor directory
    ///   normalize structure rebuild --dry-run                    # preview scope without writing
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub async fn rebuild(
        &self,
        #[param(help = "What to extract: symbols, calls, imports (comma-separated)")] include: Vec<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Only include files matching glob patterns")] only: Vec<String>,
        #[param(help = "Exclude files matching glob patterns")] exclude: Vec<String>,
        #[param(help = "Force full rebuild even if incremental is possible")] full: bool,
        #[param(
            help = "Exit non-zero if any tree-sitter grammar is missing (instead of warning and continuing)"
        )]
        strict: bool,
        #[param(help = "Dry run - show what would be rebuilt without writing the index")]
        dry_run: bool,
    ) -> Result<RebuildReport, String> {
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
        rebuild_data(
            root_path.as_deref(),
            &include,
            &only,
            &exclude,
            full,
            strict,
            dry_run,
        )
        .await
    }

    /// Show index statistics (DB size vs codebase size)
    ///
    /// Examples:
    ///   normalize structure stats              # show symbol/call/import counts and DB size
    ///   normalize structure stats --storage    # show storage usage for index and caches
    #[cli(display_with = "display_output")]
    pub async fn stats(
        &self,
        #[param(help = "Show storage usage for index and caches")] storage: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FactsStatsReport, String> {
        if storage {
            let effective_root = resolve_root(root)?;
            return Ok(FactsStatsReport::Storage(build_storage_report(
                &effective_root,
            )));
        }
        let root_path = root.map(PathBuf::from);
        stats_data(root_path.as_deref())
            .await
            .map(FactsStatsReport::Stats)
    }

    /// List indexed files (with optional prefix filter)
    ///
    /// Examples:
    ///   normalize structure files                    # list all indexed files
    ///   normalize structure files src/               # list files under src/
    ///   normalize structure files -l 10              # show only the first 10 files
    #[cli(display_with = "display_output")]
    pub async fn files(
        &self,
        #[param(positional, help = "Filter files by prefix")] prefix: Option<String>,
        #[param(short = 'l', help = "Maximum number of files to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<FileListReport, String> {
        let limit = limit.unwrap_or(100);
        let root_path = root.map(PathBuf::from);
        list_files_data(prefix.as_deref(), root_path.as_deref(), limit).await
    }

    /// Index external packages (stdlib, site-packages) into global cache
    ///
    /// Examples:
    ///   normalize structure packages                  # index all detected ecosystems
    ///   normalize structure packages --only rust      # index only Rust stdlib
    ///   normalize structure packages --clear          # clear cache and re-index
    #[cli(display_with = "display_output")]
    pub async fn packages(
        &self,
        #[param(help = "Ecosystems to index (comma-separated)")] only: Vec<String>,
        #[param(help = "Clear existing index before re-indexing")] clear: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<PackagesReport, String> {
        let root_path = root.map(PathBuf::from);
        packages_data(&only, clear, root_path.as_deref()).await
    }

    /// Run an arbitrary SQL query against the structural index
    ///
    /// Opens the index read-only and returns results as a JSON array of objects.
    /// The index exposes these tables: files, symbols (includes a `complexity`
    /// column — cyclomatic complexity, NULL for non-function/method symbols),
    /// symbol_attributes, symbol_implements, calls, imports, type_methods,
    /// type_refs, file_churn (per-file commit_count, last_changed, lines_added,
    /// lines_deleted — populated by `structure rebuild`'s co-change git walk).
    /// Three convenience views are also available:
    ///   entry_points      — public symbols with no callers
    ///   external_deps     — imports where resolved_file IS NULL
    ///   external_surface  — public symbols called from files that have external deps
    ///
    /// Examples:
    ///   normalize structure query "SELECT name, kind, file FROM symbols WHERE kind = 'function' LIMIT 10"
    ///   normalize structure query "SELECT * FROM entry_points" --json
    ///   normalize structure query "SELECT file, COUNT(*) as n FROM imports GROUP BY file ORDER BY n DESC LIMIT 5"
    #[cli(display_with = "display_output")]
    pub async fn query(
        &self,
        #[param(positional, help = "SQL query to run against the structural index")] sql: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<QueryReport, String> {
        let root = resolve_root(root)?;
        let idx = open_index(&root).await?;
        let rows = idx
            .raw_query(&sql)
            .await
            .map_err(|e| format!("Query error: {}", e))?;
        Ok(QueryReport { rows })
    }

    /// Test language extraction fixtures — verify symbols, imports, and calls are extracted
    /// correctly from source files.
    ///
    /// Discovers fixture cases in `<fixture-dir>/<lang>/<case>/` (each containing an
    /// `input.<ext>` + `expected.json`). Compares actual extraction output against the
    /// expected JSON. Use `--update` to bootstrap expected files from actual output.
    ///
    /// Examples:
    ///   normalize structure test-fixtures
    ///   normalize structure test-fixtures --lang rust
    ///   normalize structure test-fixtures --fixture-dir crates/normalize-languages/tests/fixtures
    ///   normalize structure test-fixtures --update
    #[cli(display_with = "display_output")]
    pub fn test_fixtures(
        &self,
        #[param(
            short = 'd',
            help = "Directory containing fixture cases (default: crates/normalize-languages/tests/fixtures/)"
        )]
        fixture_dir: Option<String>,
        #[param(help = "Filter to a specific language")] lang: Option<String>,
        #[param(help = "Overwrite expected.json with actual extraction (bootstrap mode)")]
        update: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ExtractionFixtureTestReport, String> {
        let effective_root = resolve_root(root)?;

        let fixture_root = match fixture_dir {
            Some(ref d) => {
                let p = Path::new(d);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    effective_root.join(p)
                }
            }
            None => {
                // Default: next to the workspace root
                let workspace_root = effective_root
                    .ancestors()
                    .find(|p| p.join("Cargo.lock").exists())
                    .unwrap_or(&effective_root)
                    .to_path_buf();
                workspace_root
                    .join("crates")
                    .join("normalize-languages")
                    .join("tests")
                    .join("fixtures")
            }
        };

        if !fixture_root.exists() {
            return Err(format!(
                "Fixture directory '{}' does not exist.",
                fixture_root.display()
            ));
        }

        let cases = crate::extraction_fixtures::discover_cases(&fixture_root, lang.as_deref())
            .map_err(|e| format!("Failed to discover fixtures: {e}"))?;

        if cases.is_empty() {
            return Err(format!(
                "No extraction fixture cases found under '{}'. \
                 Add <lang>/<case>/input.<ext> + <lang>/<case>/expected.json pairs.",
                fixture_root.display()
            ));
        }

        let case_results: Vec<ExtractionFixtureCaseResult> = cases
            .iter()
            .map(|case| {
                let r = crate::extraction_fixtures::run_case(case, update);
                ExtractionFixtureCaseResult {
                    case: r.case,
                    passed: r.passed,
                    diff: r.diff,
                }
            })
            .collect();

        let passed = case_results.iter().filter(|c| c.passed).count();
        let failed = case_results.iter().filter(|c| !c.passed).count();

        let report = ExtractionFixtureTestReport {
            fixture_dir: fixture_root.display().to_string(),
            cases: case_results,
            passed,
            failed,
            updated: update,
        };

        if failed > 0 && !update {
            let detail = self.display_output(&report);
            Err(format!("{detail}\n{failed} fixture case(s) failed"))
        } else {
            Ok(report)
        }
    }

    /// Compute live-in and live-out variable sets for each basic block in a function.
    ///
    /// Uses the CFG data stored in the index (populated by `normalize structure rebuild`)
    /// to run standard backward-dataflow liveness analysis. Requires the facts index.
    ///
    /// Also known as: variable liveness, live variable analysis, dead variable detection.
    #[cli(display_with = "display_output")]
    pub async fn liveness(
        &self,
        #[param(positional, help = "Source file path")] file: String,
        #[param(short = 'f', help = "Function name to analyse (required)")] function: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<LivenessReport, String> {
        let root_path = resolve_root(root)?;
        let idx = ensure_ready(&root_path).await?;
        let rel_file = rel_to_root(&root_path, &file);
        analyze_liveness(&idx, &rel_file, &function).await
    }

    /// Show all side-effecting constructs (await, defer, yield, resource acquire/release,
    /// channel send/receive) for functions in a source file.
    ///
    /// Uses the CFG data stored in the index (populated by `normalize structure rebuild`)
    /// to report suspension points, deferred calls, generator yields, and resource operations.
    /// Requires the facts index.
    #[cli(display_with = "display_output")]
    pub async fn effects(
        &self,
        #[param(positional, help = "Source file path")] file: String,
        #[param(
            short = 'f',
            help = "Function name to analyse (defaults to all functions)"
        )]
        function: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<EffectsReport, String> {
        let root_path = resolve_root(root)?;
        let idx = ensure_ready(&root_path).await?;
        let rel_file = rel_to_root(&root_path, &file);
        analyze_effects(&idx, &rel_file, function.as_deref()).await
    }

    /// Show type-refined exception flow for functions in a source file.
    ///
    /// Uses the CFG data stored in the index (populated by `normalize structure rebuild`)
    /// to report throw sites, the catch clauses they route to (by exception type), and any
    /// unhandled throws that escape the function. Requires the facts index.
    ///
    /// Also known as: exception analysis, throw-catch mapping, unhandled exception detection.
    #[cli(display_with = "display_output")]
    pub async fn exceptions(
        &self,
        #[param(positional, help = "Source file path")] file: String,
        #[param(
            short = 'f',
            help = "Function name to analyse (defaults to all functions)"
        )]
        function: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ExceptionsReport, String> {
        let root_path = resolve_root(root)?;
        let idx = ensure_ready(&root_path).await?;
        let rel_file = rel_to_root(&root_path, &file);
        analyze_exceptions(&idx, &rel_file, function.as_deref()).await
    }
}
