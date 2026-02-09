//! Facts management commands (file index, symbols, calls, imports).

use crate::index;
use crate::paths::get_moss_dir;
use crate::rules;
use crate::skeleton;
use clap::Subcommand;
use normalize_facts_rules_api::Relations;
use normalize_languages::external_packages;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// What to extract during indexing (files are always indexed).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum FactsContent {
    /// Skip content extraction (files only)
    None,
    /// Function and type definitions
    Symbols,
    /// Function call relationships
    Calls,
    /// Import statements
    Imports,
}

impl std::fmt::Display for FactsContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FactsContent::None => write!(f, "none"),
            FactsContent::Symbols => write!(f, "symbols"),
            FactsContent::Calls => write!(f, "calls"),
            FactsContent::Imports => write!(f, "imports"),
        }
    }
}

/// Helper for default include contents
fn default_include() -> Vec<FactsContent> {
    vec![
        FactsContent::Symbols,
        FactsContent::Calls,
        FactsContent::Imports,
    ]
}

/// Helper for default limit
fn default_limit() -> usize {
    100
}

#[derive(Subcommand, serde::Deserialize, schemars::JsonSchema)]
pub enum FactsAction {
    /// Rebuild the file index
    Rebuild {
        /// What to extract: symbols, calls, imports (default: all)
        #[arg(long, value_delimiter = ',', default_values_t = vec![FactsContent::Symbols, FactsContent::Calls, FactsContent::Imports])]
        #[serde(default = "default_include")]
        include: Vec<FactsContent>,
    },

    /// Show index statistics (DB size vs codebase size)
    Stats {
        /// Show storage usage for index and caches
        #[arg(long)]
        #[serde(default)]
        storage: bool,
    },

    /// List indexed files (with optional prefix filter)
    Files {
        /// Filter files by prefix
        prefix: Option<String>,

        /// Maximum number of files to show
        #[arg(short, long, default_value = "100")]
        #[serde(default = "default_limit")]
        limit: usize,
    },

    /// Index external packages (stdlib, site-packages) into global cache
    Packages {
        /// Ecosystems to index (python, go, js, deno, java, cpp, rust). Defaults to all available.
        #[arg(long, value_delimiter = ',')]
        #[serde(default)]
        only: Vec<String>,

        /// Clear existing index before re-indexing
        #[arg(long)]
        #[serde(default)]
        clear: bool,
    },

    /// Run compiled rule packs (dylibs) against extracted facts
    Rules {
        /// Specific rule to run (runs all if not specified)
        #[arg(long)]
        rule: Option<String>,

        /// Path to a specific rule pack dylib
        #[arg(long)]
        pack: Option<PathBuf>,

        /// List available rules instead of running them
        #[arg(long)]
        #[serde(default)]
        list: bool,
    },

    /// Run Datalog rules (.dl) against extracted facts
    Check {
        /// Path to a specific .dl rules file (auto-discovers if omitted)
        rules_file: Option<PathBuf>,

        /// List available rules instead of running them
        #[arg(long)]
        #[serde(default)]
        list: bool,
    },
}

/// Run an index management action
pub fn cmd_facts(action: FactsAction, root: Option<&Path>, json: bool) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    match action {
        FactsAction::Rebuild { include } => rt.block_on(cmd_rebuild(root, &include)),
        FactsAction::Stats { storage } => rt.block_on(cmd_stats(root, json, storage)),
        FactsAction::Files { prefix, limit } => {
            rt.block_on(cmd_list_files(prefix.as_deref(), root, limit, json))
        }
        FactsAction::Packages { only, clear } => {
            rt.block_on(cmd_packages(&only, clear, root, json))
        }
        FactsAction::Rules { rule, pack, list } => rt.block_on(cmd_rules(
            root,
            rule.as_deref(),
            pack.as_deref(),
            list,
            json,
        )),
        FactsAction::Check { rules_file, list } => {
            let effective_root = root
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let config = crate::config::NormalizeConfig::load(&effective_root);
            rt.block_on(cmd_check(
                root,
                rules_file.as_deref(),
                list,
                json,
                &config.analyze.facts_rules,
            ))
        }
    }
}

// =============================================================================
// Rebuild
// =============================================================================

async fn cmd_rebuild(root: Option<&Path>, include: &[FactsContent]) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    match index::open(&root).await {
        Ok(mut idx) => match idx.refresh().await {
            Ok(count) => {
                println!("Indexed {} files", count);

                // Any content type requires call graph extraction (parsed together)
                // "none" means files only - skip call graph
                if !include.is_empty() && !include.contains(&FactsContent::None) {
                    match idx.refresh_call_graph().await {
                        Ok(mut stats) => {
                            // Delete content types that weren't requested
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

                            let mut parts = Vec::new();
                            if stats.symbols > 0 {
                                parts.push(format!("{} symbols", stats.symbols));
                            }
                            if stats.calls > 0 {
                                parts.push(format!("{} calls", stats.calls));
                            }
                            if stats.imports > 0 {
                                parts.push(format!("{} imports", stats.imports));
                            }
                            if !parts.is_empty() {
                                println!("Indexed {}", parts.join(", "));
                            }
                        }
                        Err(e) => {
                            eprintln!("Error indexing call graph: {}", e);
                            return 1;
                        }
                    }
                }
                0
            }
            Err(e) => {
                eprintln!("Error refreshing index: {}", e);
                1
            }
        },
        Err(e) => {
            eprintln!("Error opening index: {}", e);
            1
        }
    }
}

// =============================================================================
// Stats
// =============================================================================

/// Check if a file is binary by looking for null bytes
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

async fn cmd_stats(root: Option<&Path>, json: bool, storage: bool) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // If --storage, just show storage usage
    if storage {
        return cmd_storage(&root, json);
    }

    let moss_dir = get_moss_dir(&root);
    let db_path = moss_dir.join("index.sqlite");

    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let idx = match index::open(&root).await {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Failed to open index: {}", e);
            return 1;
        }
    };

    let files = match idx.all_files().await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to read files: {}", e);
            return 1;
        }
    };

    let file_count = files.iter().filter(|f| !f.is_dir).count();
    let dir_count = files.iter().filter(|f| f.is_dir).count();

    // Count by extension (detect binary files)
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

    // Calculate codebase size
    let mut codebase_size = 0u64;
    for f in &files {
        if !f.is_dir {
            let full_path = root.join(&f.path);
            if let Ok(meta) = std::fs::metadata(&full_path) {
                codebase_size += meta.len();
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "db_size_bytes": db_size,
            "codebase_size_bytes": codebase_size,
            "ratio": if codebase_size > 0 { db_size as f64 / codebase_size as f64 } else { 0.0 },
            "file_count": file_count,
            "dir_count": dir_count,
            "symbol_count": stats.symbols,
            "call_count": stats.calls,
            "import_count": stats.imports,
            "extensions": ext_list.iter().take(20).map(|(e, c)| serde_json::json!({"ext": e, "count": c})).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Index Statistics");
        println!("================");
        println!();
        println!(
            "Database:     {} ({:.1} KB)",
            db_path.display(),
            db_size as f64 / 1024.0
        );
        println!(
            "Codebase:     {:.1} MB",
            codebase_size as f64 / 1024.0 / 1024.0
        );
        println!(
            "Ratio:        {:.2}%",
            if codebase_size > 0 {
                db_size as f64 / codebase_size as f64 * 100.0
            } else {
                0.0
            }
        );
        println!();
        println!("Files:        {} ({} dirs)", file_count, dir_count);
        println!("Symbols:      {}", stats.symbols);
        println!("Calls:        {}", stats.calls);
        println!("Imports:      {}", stats.imports);
        println!();
        println!("Top extensions:");
        for (ext, count) in ext_list.iter().take(15) {
            println!("  {:12} {:>6}", ext, count);
        }
    }

    0
}

// =============================================================================
// List Files
// =============================================================================

async fn cmd_list_files(
    prefix: Option<&str>,
    root: Option<&Path>,
    limit: usize,
    json: bool,
) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let idx = match index::open(&root).await {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Failed to open index: {}", e);
            return 1;
        }
    };

    let files = match idx.all_files().await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to read files: {}", e);
            return 1;
        }
    };

    let prefix_str = prefix.unwrap_or("");
    let filtered: Vec<&str> = files
        .iter()
        .filter(|f| !f.is_dir && f.path.starts_with(prefix_str))
        .take(limit)
        .map(|f| f.path.as_str())
        .collect();

    if json {
        println!("{}", serde_json::to_string(&filtered).unwrap());
    } else {
        for path in &filtered {
            println!("{}", path);
        }
    }

    0
}

// =============================================================================
// Packages
// =============================================================================

/// Result of indexing packages for a language
struct IndexedCounts {
    packages: usize,
    symbols: usize,
}

async fn cmd_packages(only: &[String], clear: bool, root: Option<&Path>, json: bool) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let pkg_index = match external_packages::PackageIndex::open().await {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Failed to open package index: {}", e);
            return 1;
        }
    };

    if clear {
        if let Err(e) = pkg_index.clear().await {
            eprintln!("Failed to clear index: {}", e);
            return 1;
        }
        if !json {
            println!("Cleared existing index");
        }
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
        let counts = index_language_packages(*deps, &pkg_index, &root, json).await;
        results.insert(eco_key, counts);
    }

    if json {
        let mut json_obj = serde_json::Map::new();
        for (key, counts) in &results {
            json_obj.insert(
                format!("{}_packages", key),
                serde_json::json!(counts.packages),
            );
            json_obj.insert(
                format!("{}_symbols", key),
                serde_json::json!(counts.symbols),
            );
        }
        println!("{}", serde_json::Value::Object(json_obj));
    } else {
        println!("\nIndexing complete:");
        for (key, counts) in &results {
            println!(
                "  {}: {} packages, {} symbols",
                key, counts.packages, counts.symbols
            );
        }
    }

    0
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

async fn index_language_packages(
    deps: &dyn normalize_local_deps::LocalDeps,
    pkg_index: &external_packages::PackageIndex,
    project_root: &Path,
    json: bool,
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

    if !json {
        println!(
            "Indexing {} packages (version {:?})...",
            deps.language_name(),
            version
        );
    }

    let sources = deps.dep_sources(project_root);
    if sources.is_empty() {
        if !json {
            println!("  No package sources found");
        }
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
        if !json {
            println!("  {}: {}", source.name, source.path.display());
        }

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

// =============================================================================
// Storage
// =============================================================================

/// Show storage usage for index and caches
fn cmd_storage(root: &Path, json: bool) -> i32 {
    // Project index: .normalize/index.sqlite
    let index_path = root.join(".normalize").join("index.sqlite");
    let index_size = std::fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);

    // Package cache: ~/.cache/moss/packages/
    let cache_dir = get_cache_dir().map(|d| d.join("packages"));
    let cache_size = cache_dir.as_ref().map(|d| dir_size(d)).unwrap_or(0);

    // Global cache: ~/.cache/moss/ (total)
    let global_cache_dir = get_cache_dir();
    let global_size = global_cache_dir.as_ref().map(|d| dir_size(d)).unwrap_or(0);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "index": {
                    "path": index_path.display().to_string(),
                    "bytes": index_size,
                    "human": format_size(index_size),
                },
                "package_cache": {
                    "path": cache_dir.as_ref().map(|d| d.display().to_string()),
                    "bytes": cache_size,
                    "human": format_size(cache_size),
                },
                "global_cache": {
                    "path": global_cache_dir.as_ref().map(|d| d.display().to_string()),
                    "bytes": global_size,
                    "human": format_size(global_size),
                },
                "total_bytes": index_size + global_size,
                "total_human": format_size(index_size + global_size),
            })
        );
    } else {
        println!("Storage Usage");
        println!();
        println!(
            "Project index:   {:>10}  {}",
            format_size(index_size),
            index_path.display()
        );
        if let Some(ref cache) = cache_dir {
            println!(
                "Package cache:   {:>10}  {}",
                format_size(cache_size),
                cache.display()
            );
        }
        if let Some(ref global) = global_cache_dir {
            println!(
                "Global cache:    {:>10}  {}",
                format_size(global_size),
                global.display()
            );
        }
        println!();
        println!(
            "Total:           {:>10}",
            format_size(index_size + global_size)
        );
    }

    0
}

// =============================================================================
// Rules
// =============================================================================

async fn cmd_rules(
    root: Option<&Path>,
    rule: Option<&str>,
    pack_path: Option<&Path>,
    list: bool,
    json: bool,
) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Load rule pack(s)
    let packs: Vec<rules::LoadedRulePack> = if let Some(path) = pack_path {
        // Load specific pack
        match rules::load_from_path(path) {
            Ok(pack) => vec![pack],
            Err(e) => {
                eprintln!("Error loading rule pack: {}", e);
                return 1;
            }
        }
    } else {
        // Discover and load all packs
        let results = rules::load_all(&root);
        let mut loaded = Vec::new();
        for result in results {
            match result {
                Ok(pack) => loaded.push(pack),
                Err(e) => eprintln!("Warning: {}", e),
            }
        }
        loaded
    };

    if packs.is_empty() {
        eprintln!("No rule packs found.");
        eprintln!("Search paths:");
        for path in rules::search_paths(&root) {
            eprintln!("  - {}", path.display());
        }
        eprintln!("\nTo use the builtins, copy the compiled library to one of the search paths.");
        return 1;
    }

    // List mode - just show available rules
    if list {
        if json {
            let all_rules: Vec<_> = packs
                .iter()
                .map(|pack| {
                    let info = pack.info();
                    serde_json::json!({
                        "pack_id": info.id.to_string(),
                        "pack_name": info.name.to_string(),
                        "version": info.version.to_string(),
                        "rules": info.rules.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&all_rules).unwrap());
        } else {
            for pack in &packs {
                let info = pack.info();
                println!("{} v{}", info.name, info.version);
                println!("  ID: {}", info.id);
                println!("  Path: {}", pack.path.display());
                println!("  Rules:");
                for rule_id in info.rules.iter() {
                    println!("    - {}", rule_id);
                }
                println!();
            }
        }
        return 0;
    }

    // Build relations from index
    let relations = match build_relations_from_index(&root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            eprintln!("Run `normalize facts rebuild` first to index the codebase.");
            return 1;
        }
    };

    // Run rules
    let mut all_diagnostics = Vec::new();
    let use_colors = !json && std::io::stdout().is_terminal();

    for pack in &packs {
        let diagnostics = if let Some(rule_id) = rule {
            pack.run_rule(rule_id, &relations)
        } else {
            pack.run(&relations)
        };
        all_diagnostics.extend(diagnostics);
    }

    // Output results
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all_diagnostics).unwrap()
        );
    } else if all_diagnostics.is_empty() {
        println!("No issues found.");
    } else {
        for diag in &all_diagnostics {
            println!("{}", rules::format_diagnostic(diag, use_colors));
        }
        println!("\n{} issue(s) found.", all_diagnostics.len());
    }

    if all_diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

// =============================================================================
// Check (interpreted rules)
// =============================================================================

async fn cmd_check(
    root: Option<&Path>,
    rules_file: Option<&Path>,
    list: bool,
    json: bool,
    config: &crate::interpret::FactsRulesConfig,
) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // If a specific file is given, run just that file (original behavior)
    if let Some(path) = rules_file {
        return cmd_check_file(&root, path, json).await;
    }

    // Auto-discover rules with config overrides
    let all_rules = crate::interpret::load_all_rules(&root, config);

    if list {
        if json {
            let rules_json: Vec<_> = all_rules
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "message": r.message,
                        "builtin": r.builtin,
                        "source_path": if r.source_path.as_os_str().is_empty() {
                            None
                        } else {
                            Some(r.source_path.display().to_string())
                        },
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&rules_json).unwrap());
        } else {
            let builtin_count = all_rules.iter().filter(|r| r.builtin).count();
            let project_count = all_rules.len() - builtin_count;
            println!(
                "{} fact rules ({} builtin, {} project)",
                all_rules.len(),
                builtin_count,
                project_count
            );
            println!();
            for rule in &all_rules {
                let source = if rule.builtin { "builtin" } else { "project" };
                println!("  {:30} [{}] {}", rule.id, source, rule.message);
            }
        }
        return 0;
    }

    if all_rules.is_empty() {
        println!("No fact rules found.");
        return 0;
    }

    // Build relations from index
    let relations = match build_relations_from_index(&root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            eprintln!("Run `normalize facts rebuild` first to index the codebase.");
            return 1;
        }
    };

    // Run all rules
    let mut all_diagnostics = Vec::new();
    let use_colors = !json && std::io::stdout().is_terminal();

    for rule in &all_rules {
        match crate::interpret::run_rule(rule, &relations) {
            Ok(diagnostics) => all_diagnostics.extend(diagnostics),
            Err(e) => {
                eprintln!("Error running rule '{}': {}", rule.id, e);
            }
        }
    }

    // Filter inline normalize-facts-allow: comments in source files
    crate::interpret::filter_inline_allowed(&mut all_diagnostics, &root);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all_diagnostics).unwrap()
        );
    } else if all_diagnostics.is_empty() {
        println!("No issues found ({} rules checked).", all_rules.len());
    } else {
        for diag in &all_diagnostics {
            println!("{}", rules::format_diagnostic(diag, use_colors));
        }
        println!(
            "\n{} issue(s) found ({} rules checked).",
            all_diagnostics.len(),
            all_rules.len()
        );
    }

    if all_diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

/// Run a single .dl file directly (explicit path mode)
async fn cmd_check_file(root: &Path, rules_file: &Path, json: bool) -> i32 {
    let relations = match build_relations_from_index(root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            eprintln!("Run `normalize facts rebuild` first to index the codebase.");
            return 1;
        }
    };

    let diagnostics = match crate::interpret::run_rules_file(rules_file, &relations) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };

    let use_colors = !json && std::io::stdout().is_terminal();

    if json {
        println!("{}", serde_json::to_string_pretty(&diagnostics).unwrap());
    } else if diagnostics.is_empty() {
        println!("No issues found.");
    } else {
        for diag in &diagnostics {
            println!("{}", rules::format_diagnostic(diag, use_colors));
        }
        println!("\n{} issue(s) found.", diagnostics.len());
    }

    if diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

/// Build Relations from the index
async fn build_relations_from_index(root: &Path) -> Result<Relations, String> {
    let idx = index::open(root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let mut relations = Relations::new();

    // Get symbols (file, name, kind, start_line, end_line, parent)
    let symbols = idx
        .all_symbols_with_details()
        .await
        .map_err(|e| format!("Failed to get symbols: {}", e))?;

    for (file, name, kind, start_line, _end_line, _parent) in symbols {
        relations.add_symbol(&file, &name, &kind, start_line as u32);
    }

    // Get imports (file, module, name, line)
    let imports = idx
        .all_imports()
        .await
        .map_err(|e| format!("Failed to get imports: {}", e))?;

    for (file, module, name, _line) in imports {
        relations.add_import(&file, &module, &name);
    }

    // Get calls (caller_file, caller_symbol, callee_name, line)
    let calls = idx
        .all_calls_with_lines()
        .await
        .map_err(|e| format!("Failed to get calls: {}", e))?;

    for (file, caller, callee, line) in calls {
        relations.add_call(&file, &caller, &callee, line);
    }

    Ok(relations)
}

fn get_cache_dir() -> Option<PathBuf> {
    if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
        Some(PathBuf::from(cache).join("moss"))
    } else if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(".cache").join("moss"))
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        Some(PathBuf::from(home).join(".cache").join("moss"))
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
            let path = entry.path();
            if path.is_file() {
                total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += dir_size(&path);
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
