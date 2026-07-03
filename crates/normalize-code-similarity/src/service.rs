//! CLI service for the `similarity` verb.
//!
//! Implements `normalize similarity` (duplicate/near-duplicate code detection —
//! functions, blocks, clusters), `similarity duplicate-types`, and
//! `similarity fragments` via the server-less `#[cli]` pattern.
//!
//! The service owns its config access: it loads the `[analyze]`, `[aliases]`, and
//! `[pretty]` sections directly from the global and project `config.toml` files
//! (the "sessions technique"), so this crate does not depend on the main crate's
//! monolithic `NormalizeConfig`. All compute walks the filesystem directly (no
//! daemon-aware index or import graph).

use crate::duplicates::{
    DuplicateBlocksConfig, DuplicateFunctionsConfig, DuplicateTypesReport, SimilarBlocksConfig,
    SimilarFunctionsConfig,
};
use crate::duplicates_views::{DuplicateMode, DuplicateScope, DuplicatesReport};
use crate::fragments::{FragmentScope, FragmentsReport};
use normalize_filter::{AliasConfig, Filter};
use normalize_output::{OutputFormatter, PrettyConfig};
use serde::Deserialize;
use server_less::cli;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The `[analyze]` slice this service reads for excludes / allowlists / min-lines.
///
/// Named fields consume the scalar `[analyze]` keys so the flattened `subcommands`
/// map only captures `[analyze.<subcommand>]` tables; unknown subcommand fields are
/// tolerated (value type is `toml::Value`), so this never fails to parse regardless
/// of what else lives under `[analyze]`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AnalyzeSlice {
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(flatten)]
    subcommands: HashMap<String, toml::Value>,
}

impl AnalyzeSlice {
    fn sub(&self, name: &str) -> Option<&toml::Table> {
        self.subcommands.get(name).and_then(|v| v.as_table())
    }

    fn duplicates_min_lines(&self) -> Option<usize> {
        self.sub("duplicates")
            .and_then(|t| t.get("min_lines"))
            .and_then(|v| v.as_integer())
            .map(|i| i as usize)
    }

    /// Merge global `[analyze].exclude` with `[analyze.<subcommand>].exclude`.
    fn excludes_for(&self, subcommand: &str) -> Vec<String> {
        let mut result = self.exclude.clone();
        if let Some(t) = self.sub(subcommand)
            && let Some(arr) = t.get("exclude").and_then(|v| v.as_array())
        {
            result.extend(arr.iter().filter_map(|v| v.as_str().map(String::from)));
        }
        result
    }

    fn allows_for(&self, subcommand: &str) -> Vec<String> {
        self.sub(subcommand)
            .and_then(|t| t.get("allow"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The `[analyze]` + `[aliases]` + `[pretty]` config sections this service reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SimilarityConfig {
    analyze: AnalyzeSlice,
    aliases: AliasConfig,
    pretty: PrettyConfig,
}

fn config_paths(root: &Path) -> impl Iterator<Item = PathBuf> {
    let global = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|c| c.join("normalize").join("config.toml"));
    [global, Some(root.join(".normalize").join("config.toml"))]
        .into_iter()
        .flatten()
}

/// Load the relevant config sections from the global then project `config.toml`.
///
/// Later files override earlier ones (project overrides global), matching the
/// precedence the main crate's `NormalizeConfig::load` uses.
fn load_config(root: &Path) -> SimilarityConfig {
    let mut cfg = SimilarityConfig::default();
    for path in config_paths(root) {
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(parsed) = toml::from_str::<SimilarityConfig>(&content)
        {
            cfg = parsed;
        }
    }
    cfg
}

/// Load just the `[aliases]` slice (used by the standalone `build_filter` helper).
pub(crate) fn load_aliases(root: &Path) -> AliasConfig {
    load_config(root).aliases
}

/// Discover git repositories under `dir` up to `depth` levels deep.
fn discover_repos(dir: &str, depth: usize) -> Result<Vec<PathBuf>, String> {
    let base = PathBuf::from(dir);
    if !base.is_dir() {
        return Err(format!("not a directory: {dir}"));
    }
    let mut repos = Vec::new();
    if base.join(".git").exists() {
        repos.push(base.clone());
    }
    fn walk(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) {
        if depth == 0 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.join(".git").exists() {
                        repos.push(path.clone());
                    }
                    walk(&path, depth - 1, repos);
                }
            }
        }
    }
    walk(&base, depth, &mut repos);
    repos.sort();
    repos.dedup();
    if repos.is_empty() {
        return Err(format!("no git repositories found under {dir}"));
    }
    Ok(repos)
}

/// CLI service implementing `normalize similarity` subcommands.
pub struct SimilarityService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl SimilarityService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
        }
    }

    fn root_path(root: Option<String>) -> Result<PathBuf, String> {
        root.map(PathBuf::from).map_or_else(
            || std::env::current_dir().map_err(|e| format!("failed to get working directory: {e}")),
            Ok,
        )
    }

    /// Resolve pretty mode from CLI flags plus the project's `[pretty]` config.
    fn resolve_format(&self, cfg: &SimilarityConfig) {
        let is_pretty = !self.compact_raw.get() && (self.pretty_raw.get() || cfg.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    /// Build a filter with merged excludes: config global + per-subcommand + CLI args.
    fn build_filter(
        root: &Path,
        cfg: &SimilarityConfig,
        subcommand: &str,
        cli_exclude: &[String],
        only: &[String],
    ) -> Option<Filter> {
        let mut excludes = cfg.analyze.excludes_for(subcommand);
        excludes.extend(cli_exclude.iter().cloned());
        if excludes.is_empty() && only.is_empty() {
            None
        } else {
            crate::build_filter_with_aliases(root, &cfg.aliases, &excludes, only)
        }
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_duplicates(&self, r: &DuplicatesReport) -> String {
        self.display_output(r)
    }

    fn display_dup_types(&self, r: &DuplicateTypesReport) -> String {
        self.display_output(r)
    }

    fn display_fragments(&self, r: &FragmentsReport) -> String {
        self.display_output(r)
    }
}

impl server_less::CliGlobals for SimilarityService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "similarity",
    description = "Detect duplicate and near-duplicate code: copy-paste clones, duplicate type definitions, and repeated AST fragments. Walks the filesystem directly (no index required).",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl SimilarityService {
    /// Detect duplicate or similar code blocks and functions across the codebase.
    ///
    /// Also known as: copy-paste detection, code clones, dead code candidates, duplicated
    /// logic, DRY violations. Finds code that has been copied and can be consolidated.
    ///
    /// Modes: `exact` (default) finds byte-identical bodies; `similar` uses MinHash fuzzy
    /// matching; `clusters` groups near-duplicates into connected components. Returns a
    /// `DuplicatesReport` with grouped matches and similarity scores.
    ///
    /// Groups where all items share the same name (likely trait implementations), parallel
    /// implementations across sibling directories, and same-body-pattern clusters are
    /// suppressed by default. Use `--include-trait-impls` to include them in output.
    ///
    /// Examples:
    ///   normalize similarity                       # exact duplicate functions
    ///   normalize similarity --mode similar        # fuzzy near-duplicates
    ///   normalize similarity --mode clusters       # connected-component clusters
    #[cli(default, display_with = "display_duplicates")]
    #[allow(clippy::too_many_arguments)]
    pub fn duplicates(
        &self,
        #[param(help = "Scope: functions (default) or blocks")] scope: Option<DuplicateScope>,
        #[param(help = "Detection mode: exact (default), similar (fuzzy), or clusters")]
        mode: Option<DuplicateMode>,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing (default: true for blocks scope)")]
        elide_literals: bool,
        #[param(
            help = "Keep literal values distinct when comparing (blocks scope: opt out of default elision)"
        )]
        no_elide_literals: bool,
        #[param(help = "Show source code for matches")] show_source: bool,
        #[param(help = "Minimum lines to be considered")] min_lines: Option<usize>,
        #[param(help = "Include groups where all items share the same name")]
        include_trait_impls: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(help = "Minimum similarity threshold (0.0-1.0, similar/clusters mode)")]
        similarity: Option<f64>,
        #[param(help = "Match on control-flow structure (similar/clusters mode)")] skeleton: bool,
        #[param(help = "Scan across all git repos under DIR (functions scope only)")]
        repos_dir: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
        #[param(help = "Skip function/method nodes (blocks scope only)")] skip_functions: bool,
        #[param(
            short = 'l',
            help = "Maximum number of results to show (0=no limit, clusters mode)"
        )]
        limit: Option<usize>,
    ) -> Result<DuplicatesReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let scope = scope.unwrap_or(DuplicateScope::Functions);
        let mode = mode.unwrap_or(DuplicateMode::Exact);
        let filter = Self::build_filter(&root_path, &cfg, "duplicates", &exclude, &only);
        let config_min_lines = cfg.analyze.duplicates_min_lines();

        // Blocks scope defaults to eliding literals (structurally-identical blocks that differ
        // only in literal values are real duplication). Use --no-elide-literals to opt out.
        let elide_literals = match scope {
            DuplicateScope::Blocks => !no_elide_literals,
            _ => elide_literals,
        };

        match (mode, scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos_dir {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(crate::duplicates::build_duplicate_functions_report(
                    DuplicateFunctionsConfig {
                        roots: &roots,
                        elide_identifiers,
                        elide_literals,
                        show_source,
                        min_lines: min_lines.or(config_min_lines).unwrap_or(1),
                        include_trait_impls,
                        filter: filter.as_ref(),
                        config_allow: cfg.analyze.allows_for("duplicate-functions"),
                    },
                ))
            }
            (DuplicateMode::Exact, DuplicateScope::Blocks) => Ok(
                crate::duplicates::build_duplicate_blocks_report(DuplicateBlocksConfig {
                    root: &root_path,
                    min_lines: min_lines.or(config_min_lines).unwrap_or(5),
                    elide_identifiers,
                    elide_literals,
                    skip_functions,
                    show_source,
                    allow: None,
                    reason: None,
                    filter: filter.as_ref(),
                    config_allow: cfg.analyze.allows_for("duplicate-blocks"),
                }),
            ),
            (DuplicateMode::Similar, DuplicateScope::Functions) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos_dir {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(crate::duplicates::build_similar_functions_report(
                    SimilarFunctionsConfig {
                        roots: &roots,
                        min_lines: min_lines.or(config_min_lines).unwrap_or(10),
                        similarity: similarity.unwrap_or(0.85),
                        elide_identifiers,
                        elide_literals,
                        skeleton,
                        show_source,
                        include_trait_impls,
                        allow: None,
                        reason: None,
                        filter: filter.as_ref(),
                        config_allow: cfg.analyze.allows_for("similar-functions"),
                    },
                ))
            }
            (DuplicateMode::Similar, DuplicateScope::Blocks) => Ok(
                crate::duplicates::build_similar_blocks_report(SimilarBlocksConfig {
                    root: &root_path,
                    min_lines: min_lines.or(config_min_lines).unwrap_or(15),
                    similarity: similarity.unwrap_or(0.85),
                    elide_identifiers,
                    elide_literals,
                    skeleton,
                    show_source,
                    include_trait_impls,
                    allow: None,
                    reason: None,
                    filter: filter.as_ref(),
                    config_allow: cfg.analyze.allows_for("similar-blocks"),
                }),
            ),
            (DuplicateMode::Clusters, _) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos_dir {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(crate::clusters::build_clusters_report_multi(
                    &roots,
                    min_lines.or(config_min_lines).unwrap_or(10),
                    similarity.unwrap_or(0.85),
                    elide_identifiers,
                    skeleton,
                    include_trait_impls,
                    limit.unwrap_or(20),
                    filter.as_ref(),
                ))
            }
        }
    }

    /// Detect duplicate type definitions (structs, enums, classes) across the codebase.
    ///
    /// Finds types with identical or near-identical field layouts that have been defined
    /// more than once. Returns a `DuplicateTypesReport` with grouped matches.
    #[cli(display_with = "display_dup_types")]
    pub fn duplicate_types(
        &self,
        #[param(positional, help = "Target directory to scan")] target: Option<String>,
        #[param(help = "Minimum field overlap percentage")] min_overlap: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<DuplicateTypesReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let scan_root = target
            .map(PathBuf::from)
            .unwrap_or_else(|| root_path.clone());
        let config_allow = cfg.analyze.allows_for("duplicate-types");
        Ok(crate::duplicates::build_duplicate_types_report(
            &scan_root,
            &root_path,
            min_overlap.unwrap_or(70),
            &config_allow,
        ))
    }

    /// Find repeated AST fragments (sub-expressions, statement patterns) across the codebase.
    ///
    /// Operates at a finer granularity than `duplicates`: finds repeated structural patterns
    /// within function bodies rather than entire functions. The `scope` parameter controls
    /// whether to search within functions or across blocks. Returns a `FragmentsReport`.
    #[cli(display_with = "display_fragments")]
    #[allow(clippy::too_many_arguments)]
    pub fn fragments(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Minimum subtree size to consider (default: 10)")]
        min_nodes: Option<usize>,
        #[param(
            short = 's',
            help = "What to hash: all|functions|blocks (default: all)"
        )]
        scope: Option<String>,
        #[param(
            short = 'e',
            help = "Only analyze symbols matching unified path glob (requires --scope functions)"
        )]
        entry: Option<String>,
        #[param(help = "Resolve calls and inline callee bodies (default: 0, requires index)")]
        inline_depth: Option<usize>,
        #[param(help = "MinHash similarity threshold for fuzzy grouping (default: 1.0 = exact)")]
        similarity: Option<f64>,
        #[param(short = 'l', help = "Max clusters to report (0=no limit, default: 30)")]
        limit: Option<usize>,
        #[param(help = "Match on control-flow structure only")] skeleton: bool,
        #[param(short = 'm', help = "Minimum cluster size to report (default: 2)")]
        min_members: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
    ) -> Result<FragmentsReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let scope_val: FragmentScope = scope
            .as_deref()
            .unwrap_or("all")
            .parse()
            .map_err(|e: String| e)?;
        let mut merged_exclude = cfg.analyze.excludes_for("fragments");
        merged_exclude.extend(exclude);
        crate::fragments::analyze_fragments(
            &root_path,
            min_nodes.unwrap_or(10),
            scope_val,
            entry.as_deref(),
            inline_depth.unwrap_or(0),
            similarity.unwrap_or(1.0),
            limit.unwrap_or(30),
            skeleton,
            min_members.unwrap_or(2),
            &merged_exclude,
            &only,
        )
    }
}
