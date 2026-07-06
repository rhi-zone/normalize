//! CLI service for the `history` verb.
//!
//! Implements `normalize history` and its subcommands — churn hotspots,
//! temporal coupling, blame ownership, contributors, activity-over-time,
//! repo-coupling, and change-coupling clusters — via the server-less `#[cli]`
//! pattern.
//!
//! Scope note: this is the repo-wide, cross-file *statistical* history verb. It
//! is distinct from `view history` (single-file git chronology / log), which
//! stays in the main crate. The two coexist.
//!
//! The service owns its config access: it loads the `[analyze]` (exclude
//! patterns), `[index]`, `[walk]`, and `[pretty]` sections directly from the
//! global and project `config.toml` files, so this crate does not depend on the
//! main crate's monolithic `NormalizeConfig`. The `coupling-clusters` command
//! loads co-change edges from the structural index via
//! `normalize_index::ensure_ready_or_warn` (which takes the config **slices**),
//! falling back to a git-history walk.

use crate::activity::{ActivityReport, WindowGranularity};
use crate::contributors::ContributorsReport;
use crate::coupling::CouplingReport;
use crate::coupling_clusters::CouplingClustersReport;
use crate::hotspots::{HotspotsRepoEntry, HotspotsReport};
use crate::ownership::{OwnershipRepoEntry, OwnershipReport};
use crate::repo_coupling::RepoCouplingReport;
use normalize_index::IndexConfig;
use normalize_output::{OutputFormatter, PrettyConfig};
use normalize_rules_config::WalkConfig;
use server_less::cli;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Config slices this service reads.
///
/// `[index]`, `[walk]`, and `[pretty]` deserialize cleanly via serde (unknown
/// top-level sections are ignored). The `[analyze]` excludes are extracted
/// manually because `[analyze]` mixes scalar keys (`threshold`, `compact`, …)
/// with `[analyze.<subcommand>]` tables — a serde `flatten` would choke on the
/// scalars.
#[derive(Default)]
struct HistoryConfig {
    index: IndexConfig,
    walk: WalkConfig,
    pretty_enabled: bool,
    /// `[analyze] exclude` — applied to every subcommand.
    global_exclude: Vec<String>,
    /// `[analyze] hotspots_exclude` — additional excludes for hotspots.
    hotspots_exclude: Vec<String>,
    /// `[analyze.<subcommand>] exclude` — per-subcommand extra excludes.
    sub_exclude: HashMap<String, Vec<String>>,
}

impl HistoryConfig {
    /// Merge global + per-subcommand excludes (CLI `--exclude` appended by caller).
    fn excludes_for(&self, subcommand: &str) -> Vec<String> {
        let mut result = self.global_exclude.clone();
        if let Some(extra) = self.sub_exclude.get(subcommand) {
            result.extend(extra.iter().cloned());
        }
        result
    }
}

/// `[index]` + `[walk]` + `[pretty]` deserialized via serde.
#[derive(serde::Deserialize, Default)]
struct SliceConfig {
    #[serde(default)]
    index: IndexConfig,
    #[serde(default)]
    walk: WalkConfig,
    #[serde(default)]
    pretty: PrettyConfig,
}

fn string_array(value: Option<&toml::Value>) -> Vec<String> {
    match value {
        Some(toml::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Load the relevant config sections from the global then project `config.toml`.
///
/// Later files override earlier ones (project overrides global) on a per-field
/// basis, matching the precedence the main crate's `NormalizeConfig::load` uses.
fn load_config(root: &Path) -> HistoryConfig {
    let global = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|c| c.join("normalize").join("config.toml"));

    let mut cfg = HistoryConfig::default();
    for path in [global, Some(root.join(".normalize").join("config.toml"))]
        .into_iter()
        .flatten()
    {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(table) = toml::from_str::<toml::Table>(&content) else {
            continue;
        };

        // Clean serde slices for [index]/[walk]/[pretty].
        if let Ok(slice) = toml::Value::Table(table.clone()).try_into::<SliceConfig>() {
            cfg.index = slice.index;
            cfg.walk = slice.walk;
            cfg.pretty_enabled = slice.pretty.enabled();
        }

        // Manual extraction of [analyze] excludes.
        if let Some(toml::Value::Table(analyze)) = table.get("analyze") {
            if analyze.contains_key("exclude") {
                cfg.global_exclude = string_array(analyze.get("exclude"));
            }
            if analyze.contains_key("hotspots_exclude") {
                cfg.hotspots_exclude = string_array(analyze.get("hotspots_exclude"));
            }
            for (key, value) in analyze {
                if let toml::Value::Table(sub) = value
                    && sub.contains_key("exclude")
                {
                    cfg.sub_exclude
                        .insert(key.clone(), string_array(sub.get("exclude")));
                }
            }
        }
    }
    cfg
}

/// Discover git repositories up to `max_depth` levels deep under `dir`.
///
/// Scans subdirectories for `.git/` directories, skipping hidden dirs. Stops
/// recursing into a directory once a `.git` is found (no nested repos).
fn discover_repos(dir: &str, max_depth: usize) -> Result<Vec<PathBuf>, String> {
    fn collect(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) -> std::io::Result<()> {
        if depth == 0 {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            if name.to_str().unwrap_or("").starts_with('.') {
                continue;
            }
            if path.join(".git").is_dir() {
                repos.push(path);
            } else if depth > 1 {
                collect(&path, depth - 1, repos)?;
            }
        }
        Ok(())
    }

    let mut repos = Vec::new();
    collect(&PathBuf::from(dir), max_depth, &mut repos)
        .map_err(|e| format!("Failed to discover repos in {dir}: {e}"))?;
    repos.sort();
    Ok(repos)
}

/// CLI service implementing `normalize history` subcommands.
pub struct HistoryService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl Default for HistoryService {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryService {
    pub fn new() -> Self {
        Self {
            pretty: Cell::new(false),
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
    fn resolve_format(&self, cfg: &HistoryConfig) {
        let is_pretty = !self.compact_raw.get() && (self.pretty_raw.get() || cfg.pretty_enabled);
        self.pretty.set(is_pretty);
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_hotspots(&self, r: &HotspotsReport) -> String {
        self.display_output(r)
    }

    fn display_coupling(&self, r: &CouplingReport) -> String {
        self.display_output(r)
    }

    fn display_ownership(&self, r: &OwnershipReport) -> String {
        self.display_output(r)
    }

    fn display_contributors(&self, r: &ContributorsReport) -> String {
        self.display_output(r)
    }

    fn display_activity(&self, r: &ActivityReport) -> String {
        self.display_output(r)
    }

    fn display_repo_coupling(&self, r: &RepoCouplingReport) -> String {
        self.display_output(r)
    }

    fn display_coupling_clusters(&self, r: &CouplingClustersReport) -> String {
        self.display_output(r)
    }
}

impl server_less::CliGlobals for HistoryService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "history",
    description = "Statistical code-health signals derived from git history: churn hotspots, temporal coupling, blame ownership, contributors, activity, and cross-repo coupling. (For a single file's git log, see `view history`.)",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl HistoryService {
    /// Rank files by churn × complexity: the highest-risk files for introducing bugs.
    ///
    /// Also known as: technical debt hotspots, bug-prone files, high-churn files, risky code.
    /// Combines git churn (commit frequency) with cyclomatic complexity to surface
    /// files that change often and are hard to reason about. Use `recency` to weight
    /// recent commits higher. Returns a `HotspotsReport` with per-file risk scores.
    ///
    /// **Score formula (without `--recency`):** `commits × √churn × log₂(1 + max_complexity)`.
    /// High scores indicate complex, bug-prone files that change often.
    ///
    /// **Score formula (with `--recency`):** `Σ(e^(-λ·age) × √churn_i) × log₂(1 + max_complexity)`,
    /// where λ = ln(2)/180 (half-life of 180 days). Recent changes are weighted higher.
    /// The `Churn` column is the total lines added + deleted across all commits.
    #[cli(display_with = "display_hotspots")]
    pub fn hotspots(
        &self,
        #[param(help = "Weight recent changes higher (exponential decay)")] recency: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Run across all git repos under DIR")] repos_dir: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<HotspotsReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        // Merge: global excludes + [analyze.hotspots] excludes + hotspots_exclude
        let mut excludes = cfg.excludes_for("hotspots");
        excludes.extend(cfg.hotspots_exclude.clone());
        if let Some(repos_dir) = repos_dir {
            let repo_paths = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
            let entries: Vec<HotspotsRepoEntry> = repo_paths
                .into_iter()
                .map(|repo_path| {
                    let name = repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    match crate::hotspots::analyze_hotspots(&repo_path, &excludes, recency) {
                        Ok(r) => HotspotsRepoEntry {
                            name,
                            error: None,
                            hotspots: r.hotspots,
                            has_complexity: r.has_complexity,
                            recency_weighted: r.recency_weighted,
                        },
                        Err(e) => HotspotsRepoEntry {
                            name,
                            error: Some(e),
                            hotspots: vec![],
                            has_complexity: false,
                            recency_weighted: recency,
                        },
                    }
                })
                .collect();
            return Ok(HotspotsReport {
                hotspots: vec![],
                has_complexity: false,
                recency_weighted: recency,
                repos: Some(entries),
            });
        }
        crate::hotspots::analyze_hotspots(&root_path, &excludes, recency)
    }

    /// Rank file pairs by temporal coupling: pairs that appear in the same git commits.
    ///
    /// Also known as: co-change analysis, change coupling, logical coupling, implicit coupling,
    /// hidden dependencies. High coupling scores indicate implicit dependencies not visible in
    /// the import graph. `min_commits` sets the minimum shared-commit threshold. Returns
    /// a `CouplingReport` with ranked pairs and their shared-commit counts.
    ///
    /// The `Confidence` column is `shared commits / max(commits_a, commits_b)`. High coupling
    /// may indicate hidden dependencies or shotgun surgery (one logical change spread across
    /// many files).
    #[cli(display_with = "display_coupling")]
    #[allow(clippy::too_many_arguments)]
    pub fn coupling(
        &self,
        #[param(help = "Minimum shared commits for coupling edges")] min_commits: Option<usize>,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<CouplingReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let mut merged_exclude = cfg.excludes_for("coupling");
        merged_exclude.extend(exclude);
        let min = min_commits.unwrap_or(3);
        let lim = limit.unwrap_or(20);
        let mut report = crate::coupling::analyze_coupling(&root_path, min, lim, &merged_exclude)?;
        if let Some(ref diff_ref) = diff {
            use normalize_git::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                crate::coupling::analyze_coupling(wt, min, usize::MAX, &merged_exclude)
            })?;
            compute_ranked_diff(&mut report.pairs, &baseline.pairs);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank files by ownership concentration: how many authors contribute to each file.
    ///
    /// Uses git blame to compute the fraction of lines owned by the top contributor.
    /// High concentration (single-author files) indicates bus-factor risk. Returns an
    /// `OwnershipReport` with per-file scores and optional cross-repo aggregation.
    ///
    /// The `Bus Factor` column is the number of authors needed to cover >50% of a file's
    /// lines. A bus factor of 1 means the file has a single effective owner — a knowledge
    /// concentration risk.
    #[cli(display_with = "display_ownership")]
    pub fn ownership(
        &self,
        #[param(short = 'l', help = "Maximum number of files to show (0=no limit)")] limit: Option<
            usize,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Run across all git repos under DIR")] repos_dir: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<OwnershipReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let mut merged_exclude = cfg.excludes_for("ownership");
        merged_exclude.extend(exclude);
        let lim = limit.unwrap_or(20);
        if let Some(repos_dir) = repos_dir {
            let repo_paths = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
            let entries: Vec<OwnershipRepoEntry> = repo_paths
                .into_iter()
                .map(|repo_path| {
                    let name = repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    match crate::ownership::analyze_ownership(&repo_path, lim, &merged_exclude) {
                        Ok(r) => OwnershipRepoEntry {
                            name,
                            error: None,
                            files: r.files,
                        },
                        Err(e) => OwnershipRepoEntry {
                            name,
                            error: Some(e),
                            files: vec![],
                        },
                    }
                })
                .collect();
            return Ok(OwnershipReport {
                files: vec![],
                repos: Some(entries),
                diff_ref: None,
            });
        }
        let mut report = crate::ownership::analyze_ownership(&root_path, lim, &merged_exclude)?;
        if let Some(ref diff_ref) = diff {
            use normalize_git::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                crate::ownership::analyze_ownership(wt, usize::MAX, &merged_exclude)
            })?;
            compute_ranked_diff(&mut report.files, &baseline.files);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank contributors across multiple repositories by commit share and overlap.
    ///
    /// Discovers git repos under `repos_dir` and aggregates per-author commit counts,
    /// surfacing cross-repo overlap (authors active in many repos). Returns a
    /// `ContributorsReport`.
    #[cli(display_with = "display_contributors")]
    pub fn contributors(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ContributorsReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        let cfg = load_config(&std::env::current_dir().unwrap_or_default());
        self.resolve_format(&cfg);
        crate::contributors::analyze_contributors(&repos)
    }

    /// Show commit activity across multiple repositories over time windows.
    ///
    /// Discovers git repos under `repos_dir`, groups commits by `window` (month or week),
    /// and returns an `ActivityReport` with per-repo commit counts across `windows` periods.
    /// Useful for identifying which repos are most actively developed.
    #[cli(display_with = "display_activity")]
    pub fn activity(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window granularity: month (default) or week")] window: Option<
            WindowGranularity,
        >,
        #[param(help = "Number of windows to show")] windows: Option<usize>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ActivityReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        let cfg = load_config(&std::env::current_dir().unwrap_or_default());
        self.resolve_format(&cfg);
        crate::activity::analyze_activity(&repos, window.unwrap_or_default(), windows.unwrap_or(12))
    }

    /// Detect temporal coupling between repositories: pairs that receive commits together.
    ///
    /// Groups commits within `window` hours as "co-changes" and reports repo pairs that
    /// appear together in at least `min_windows` co-change windows. Returns a
    /// `RepoCouplingReport` with ranked repo pairs and their co-change counts.
    #[cli(name = "repo-coupling", display_with = "display_repo_coupling")]
    pub fn repo_coupling(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window size in hours for temporal grouping")] window: Option<usize>,
        #[param(help = "Minimum shared windows to report a temporal pair")] min_windows: Option<
            usize,
        >,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<RepoCouplingReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        let cfg = load_config(&std::env::current_dir().unwrap_or_default());
        self.resolve_format(&cfg);
        crate::repo_coupling::analyze_repo_coupling(
            &repos,
            window.unwrap_or(24),
            min_windows.unwrap_or(3),
        )
    }

    /// Find clusters of files that change together in git history (connected components).
    ///
    /// Also known as: co-change analysis, change coupling, logical coupling, implicit coupling.
    /// Groups files into clusters using temporal coupling edges weighted by shared commit
    /// count. `min_commits` controls the edge threshold (auto-scaled by repo size if
    /// omitted). Returns a `CouplingClustersReport` with cluster membership and sizes.
    ///
    /// Loads co-change edges from the structural index (`co_change_edges`, populated by
    /// `normalize structure rebuild`) when available, falling back to a direct git-history
    /// walk with a warning.
    #[cli(name = "coupling-clusters", display_with = "display_coupling_clusters")]
    #[allow(clippy::too_many_arguments)]
    pub fn coupling_clusters(
        &self,
        #[param(help = "Minimum shared commits for cluster edges")] min_commits: Option<usize>,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
    ) -> Result<CouplingClustersReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let mut merged_exclude = cfg.excludes_for("coupling-clusters");
        merged_exclude.extend(exclude);

        // Auto-scale the edge threshold by repo size when not given.
        let effective_min = min_commits.unwrap_or_else(|| {
            let total = (|| -> Option<usize> {
                let repo = normalize_git::open_repo(&root_path)?;
                let head_id = repo.head_id().ok()?;
                let walk = head_id.ancestors().all().ok()?;
                Some(walk.filter(|r| r.is_ok()).count())
            })()
            .unwrap_or(60);
            (total / 20).clamp(3, 50)
        });

        // Try to load co-change edges from the structural index first. This is a
        // sync method running inside a tokio runtime, so use `block_in_place` to
        // safely drive async index I/O from a sync context.
        let index_edges: Option<Vec<(String, String, usize)>> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let idx = normalize_index::ensure_ready_or_warn(&root_path, &cfg.index, &cfg.walk)
                    .await?;
                idx.query_co_change_edges(effective_min)
                    .await
                    .ok()
                    .flatten()
            })
        });

        let raw_pairs: Vec<(String, String, usize)> = if let Some(edges) = index_edges {
            edges
        } else {
            tracing::warn!(
                "co_change_edges table is empty — falling back to git history walk. \
                 Run `normalize structure rebuild` to pre-compute the co-change index."
            );
            let coupling = crate::coupling::analyze_coupling(
                &root_path,
                effective_min,
                usize::MAX,
                &merged_exclude,
            )?;
            coupling
                .pairs
                .iter()
                .map(|p| (p.file_a.clone(), p.file_b.clone(), p.shared_commits))
                .collect()
        };

        Ok(crate::coupling_clusters::cluster_from_edges(
            raw_pairs,
            limit.unwrap_or(20),
            &merged_exclude,
            &only,
        ))
    }
}
