//! Analyze sub-service for server-less CLI.

use crate::commands::analyze::activity::ActivityReport;
use crate::commands::analyze::architecture::ArchitectureReport;
use crate::commands::analyze::coupling_clusters::CouplingClustersReport;
use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::repo_coupling::RepoCouplingReport;
use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
use crate::commands::analyze::summary::SummaryReport;
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Errors returned by the analyze service.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzeError {
    /// The index was not found; run `normalize structure rebuild` first.
    #[error("no index found; run `normalize structure rebuild` first")]
    IndexNotFound,
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// An error with a descriptive message (forwarded as-is).
    #[error("{0}")]
    Message(String),
}

impl From<String> for AnalyzeError {
    fn from(s: String) -> Self {
        AnalyzeError::Message(s)
    }
}

fn discover_repos(dir: &str, depth: usize) -> Result<Vec<PathBuf>, AnalyzeError> {
    crate::multi_repo::discover_repos_depth(&PathBuf::from(dir), depth)
        .map_err(AnalyzeError::Message)
}

/// Analyze sub-service (health, complexity, security, duplicates, docs).
pub struct AnalyzeService {
    pretty: Cell<bool>,
}

impl AnalyzeService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
        }
    }

    fn root_path(root: Option<String>) -> Result<PathBuf, AnalyzeError> {
        root.map(PathBuf::from).map_or_else(
            || {
                std::env::current_dir().map_err(|e| {
                    AnalyzeError::Message(format!("failed to get working directory: {e}"))
                })
            },
            Ok,
        )
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    /// Generic display bridge: routes to `format_pretty()` or `format_text()` based on pretty mode.
    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn build_filter(
        root: &std::path::Path,
        exclude: &[String],
        only: &[String],
    ) -> Option<crate::filter::Filter> {
        if exclude.is_empty() && only.is_empty() {
            None
        } else {
            crate::commands::build_filter(root, exclude, only)
        }
    }

    /// Build a filter with merged excludes: config global + per-subcommand + CLI args.
    fn build_filter_with_config(
        root: &std::path::Path,
        config: &crate::commands::analyze::AnalyzeConfig,
        subcommand: &str,
        cli_exclude: &[String],
        only: &[String],
    ) -> Option<crate::filter::Filter> {
        let mut excludes = config.excludes_for(subcommand);
        excludes.extend(cli_exclude.iter().cloned());
        Self::build_filter(root, &excludes, only)
    }
}

#[cli(
    name = "analyze",
    description = "Analyze codebase (health, complexity, security, duplicates, docs)",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
#[server(groups(
    code = "Code quality",
    modules = "Module structure",
    repo = "Repository",
    graph = "Graph analysis",
    git = "Git history",
    test = "Testing",
    security = "Security",
    diff = "Diff",
))]
impl AnalyzeService {
    /// Analyze architectural structure: coupling, dependency cycles, and hub modules.
    ///
    /// Detects circular imports (dependency cycles), highly-coupled module pairs, and hub
    /// modules (high fan-in or fan-out). Also known as: circular dependency detection,
    /// architecture health, import cycle finder, god module detection.
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns an `ArchitectureReport`
    /// with coupling pairs, cycle lists, and hub modules ranked by fan-in/fan-out.
    #[server(group = "graph")]
    #[cli(display_with = "display_output")]
    pub async fn architecture(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'l',
            help = "Maximum number of cross-import entries to include in output (0 = no limit, default 20)"
        )]
        limit: Option<usize>,
    ) -> Result<ArchitectureReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        let idx = crate::index::ensure_ready(&root_path).await?;
        let mut report = crate::commands::analyze::architecture::analyze_architecture(&idx)
            .await
            .map_err(|e| AnalyzeError::Message(format!("Architecture analysis failed: {}", e)))?;
        // Cap cross_imports to avoid bloated JSON output for agents.
        // Default cap is 20; --limit 0 disables the cap.
        let cap = match limit.unwrap_or(20) {
            0 => usize::MAX,
            n => n,
        };
        report.cross_imports.truncate(cap);
        Ok(report)
    }

    /// Run health analysis (file counts, complexity stats, large file warnings)
    #[cli(default, display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub fn health(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(
            short = 'l',
            help = "Maximum number of large files to include in output (0 = no limit, default 10)"
        )]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<AnalyzeReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        // Validate target path exists (catches typos and unknown subcommands routed here via #[cli(default)])
        if let Some(ref t) = target {
            let candidate = root_path.join(t);
            if !candidate.exists() && !t.contains('*') && !t.contains('?') && !t.contains('[') {
                return Err(AnalyzeError::Message(format!("path not found: {t}")));
            }
        }
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter =
            Self::build_filter_with_config(&root_path, &config.analyze, "health", &exclude, &only);
        let mut report = crate::commands::analyze::report::analyze(
            target.as_deref(),
            &root_path,
            true,
            false,
            false,
            false,
            None,
            None,
            filter.as_ref(),
        );
        // Cap large_files to avoid bloated JSON output for agents.
        // Default cap is 10; --limit 0 disables the cap.
        let cap = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        if let Some(ref mut health) = report.health {
            health.large_files.truncate(cap);
        }
        Ok(report)
    }

    /// Run all analysis passes
    #[cli(display_with = "display_output")]
    pub fn all(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<AnalyzeReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter =
            Self::build_filter_with_config(&root_path, &config.analyze, "all", &exclude, &only);
        Ok(crate::commands::analyze::report::analyze(
            target.as_deref(),
            &root_path,
            true, // health
            true, // complexity
            true, // length
            true, // security
            None,
            None,
            filter.as_ref(),
        ))
    }

    /// Scan the codebase for security issues (hardcoded secrets, unsafe patterns).
    ///
    /// Also known as: secrets detection, credential scanning, hardcoded password finder,
    /// API key leaks, vulnerability scanning, unsafe code audit.
    ///
    /// Runs heuristic pattern matching across all indexed files. The optional `target`
    /// parameter filters findings to paths that contain the given substring. Returns a
    /// `SecurityReport` with ranked findings including file, line, and severity.
    #[server(group = "security")]
    #[cli(display_with = "display_output")]
    pub fn security(
        &self,
        #[param(positional, help = "Target file or directory to filter results by")] target: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<SecurityReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        let mut report = crate::commands::analyze::security::analyze_security(&root_path);
        if let Some(t) = target {
            report.findings.retain(|f| f.file.contains(&t));
        }
        Ok(report)
    }

    /// Measure documentation coverage: which public symbols lack doc comments.
    ///
    /// Finds undocumented public symbols, missing docstrings, and documentation gaps.
    /// Also known as: doc coverage, missing documentation, undocumented API surface.
    ///
    /// Returns a `DocCoverageReport` listing files ranked by undocumented public symbols,
    /// with per-file and overall coverage percentages. Respects the `exclude_interface_impls`
    /// config option to skip auto-generated trait implementations.
    #[server(group = "repo")]
    #[cli(display_with = "display_output")]
    pub async fn docs(
        &self,
        #[param(short = 'l', help = "Maximum number of files to show (0=no limit)")] limit: Option<
            usize,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
    ) -> Result<DocCoverageReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter =
            Self::build_filter_with_config(&root_path, &config.analyze, "docs", &exclude, &only);
        Ok(crate::commands::analyze::docs::analyze_docs(
            &root_path,
            limit.unwrap_or(10),
            config.analyze.exclude_interface_impls(),
            filter.as_ref(),
        )
        .await)
    }

    /// Find clusters of files that change together in git history (connected components).
    ///
    /// Also known as: co-change analysis, change coupling, logical coupling, implicit coupling.
    /// Groups files that share frequent commits — a sign of hidden dependencies not captured
    /// in the import graph. Useful for refactoring planning and understanding blast radius.
    ///
    /// Groups files into clusters using temporal coupling edges weighted by shared commit
    /// count. `min_commits` controls the edge threshold (auto-scaled by repo size if
    /// omitted). Returns a `CouplingClustersReport` with cluster membership and sizes.
    #[server(group = "git")]
    #[cli(display_with = "display_output")]
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
        pretty: bool,
        compact: bool,
    ) -> Result<CouplingClustersReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut merged_exclude = config.analyze.excludes_for("coupling-clusters");
        merged_exclude.extend(exclude);
        let effective_min = min_commits.unwrap_or_else(|| {
            // Count total commits via gix commit walk (no PATH dependency).
            let total = (|| -> Option<usize> {
                let repo = crate::commands::analyze::git_utils::open_repo(&root_path)?;
                let head_id = repo.head_id().ok()?;
                let walk = head_id.ancestors().all().ok()?;
                Some(walk.filter(|r| r.is_ok()).count())
            })()
            .unwrap_or(60);
            (total / 20).clamp(3, 50)
        });
        crate::commands::analyze::coupling_clusters::analyze_coupling_clusters(
            &root_path,
            effective_min,
            limit.unwrap_or(20),
            &merged_exclude,
            &only,
        )
        .map_err(AnalyzeError::from)
    }

    /// Show commit activity across multiple repositories over time windows.
    ///
    /// Discovers git repos under `repos_dir`, groups commits by `window` (month or week),
    /// and returns an `ActivityReport` with per-repo commit counts across `windows` periods.
    /// Useful for identifying which repos are most actively developed.
    #[server(group = "git")]
    #[cli(display_with = "display_output")]
    pub fn activity(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window granularity: month (default) or week")] window: Option<
            crate::commands::analyze::activity::WindowGranularity,
        >,
        #[param(help = "Number of windows to show")] windows: Option<usize>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ActivityReport, AnalyzeError> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        crate::commands::analyze::activity::analyze_activity(
            &repos,
            window.unwrap_or_default(),
            windows.unwrap_or(12),
        )
        .map_err(AnalyzeError::from)
    }

    /// Detect temporal coupling between repositories: pairs that receive commits together.
    ///
    /// Groups commits within `window` hours as "co-changes" and reports repo pairs that
    /// appear together in at least `min_windows` co-change windows. Returns a
    /// `RepoCouplingReport` with ranked repo pairs and their co-change counts.
    #[server(group = "git")]
    #[cli(display_with = "display_output")]
    pub fn repo_coupling(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window size in hours for temporal grouping")] window: Option<usize>,
        #[param(help = "Minimum shared windows to report a temporal pair")] min_windows: Option<
            usize,
        >,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<RepoCouplingReport, AnalyzeError> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        crate::commands::analyze::repo_coupling::analyze_repo_coupling(
            &repos,
            window.unwrap_or(24),
            min_windows.unwrap_or(3),
        )
        .map_err(AnalyzeError::from)
    }

    /// Rank repositories by composite tech-debt score (churn × complexity × coupling).
    ///
    /// Discovers git repos under `repos_dir` and computes a health score for each by
    /// combining churn rate, average cyclomatic complexity, and temporal coupling density.
    /// Returns a `CrossRepoHealthReport` with repos ranked worst-first.
    #[server(group = "git")]
    #[cli(display_with = "display_output")]
    pub fn cross_repo_health(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<CrossRepoHealthReport, AnalyzeError> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        Ok(crate::commands::analyze::cross_repo_health::analyze_cross_repo_health(&repos))
    }

    /// Auto-generated single-page codebase overview
    #[cli(display_with = "display_output")]
    pub async fn summary(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'l',
            help = "Maximum number of worst modules to show in concerns (0=no limit)"
        )]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<SummaryReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(5) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::summary::analyze_summary(&root_path, effective_limit).await)
    }

    /// Show structural (skeleton) changes between a base ref and HEAD.
    ///
    /// Computes the skeleton (symbol signatures) at `base` and at HEAD, then diffs them
    /// to show added, removed, and changed symbols without requiring a full source diff.
    /// Returns a `SkeletonDiffReport` grouped by file with before/after signatures.
    #[server(group = "diff")]
    #[cli(display_with = "display_output")]
    pub fn skeleton_diff(
        &self,
        #[param(positional, help = "Base ref to diff against (branch, commit, HEAD~N)")]
        base: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SkeletonDiffReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut merged_exclude = config.analyze.excludes_for("skeleton-diff");
        merged_exclude.extend(exclude);
        crate::commands::analyze::skeleton_diff::analyze_skeleton_diff(
            &root_path,
            &base,
            &merged_exclude,
            &only,
        )
        .map_err(AnalyzeError::from)
    }
}
