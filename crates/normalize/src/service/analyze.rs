//! Analyze sub-service for server-less CLI.

use crate::analyze::function_length::LengthReport;
use crate::analyze::test_gaps::TestGapsReport;
use crate::commands::analyze::activity::ActivityReport;
use crate::commands::analyze::architecture::ArchitectureReport;
use crate::commands::analyze::coupling_clusters::CouplingClustersReport;
use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::repo_coupling::RepoCouplingReport;
use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
use crate::commands::analyze::summary::SummaryReport;
use crate::commands::analyze::trend::{ScalarTrendReport, TrendReport};
use crate::commands::syntax::node_types::NodeTypesReport;
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

fn discover_repos(dir: &str, depth: usize) -> Result<Vec<PathBuf>, String> {
    crate::multi_repo::discover_repos_depth(&PathBuf::from(dir), depth)
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

    fn root_path(root: Option<String>) -> PathBuf {
        root.map(PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    fn display_architecture(&self, r: &ArchitectureReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_report(&self, r: &AnalyzeReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_security(&self, r: &SecurityReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_scalar_trend(&self, r: &ScalarTrendReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_length(&self, r: &LengthReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_doc_coverage(&self, r: &DocCoverageReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_activity(&self, r: &ActivityReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_repo_coupling(&self, r: &RepoCouplingReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_cross_repo_health(&self, r: &CrossRepoHealthReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_summary(&self, r: &SummaryReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_skeleton_diff(&self, r: &SkeletonDiffReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_trend(&self, r: &TrendReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_test_gaps(&self, r: &TestGapsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_coupling_clusters(&self, r: &CouplingClustersReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_node_types(&self, r: &NodeTypesReport) -> String {
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
    description = "Analyze codebase (health, complexity, security, duplicates, docs)"
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
    /// Show architecture analysis (coupling, cycles, hubs)
    #[server(group = "graph")]
    #[cli(display_with = "display_architecture")]
    pub async fn architecture(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ArchitectureReport, String> {
        let root_path = Self::root_path(root);
        let idx = crate::index::ensure_ready(&root_path).await?;
        crate::commands::analyze::architecture::analyze_architecture(&idx)
            .await
            .map_err(|e| format!("Architecture analysis failed: {}", e))
    }

    /// Run health analysis (file counts, complexity stats, large file warnings)
    #[cli(default, display_with = "display_report")]
    pub fn health(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<AnalyzeReport, String> {
        let root_path = Self::root_path(root);
        // Validate target path exists (catches typos and unknown subcommands routed here via #[cli(default)])
        if let Some(ref t) = target {
            let candidate = root_path.join(t);
            if !candidate.exists() && !t.contains('*') && !t.contains('?') && !t.contains('[') {
                return Err(format!("path not found: {t}"));
            }
        }
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter =
            Self::build_filter_with_config(&root_path, &config.analyze, "health", &exclude, &only);
        Ok(crate::commands::analyze::report::analyze(
            target.as_deref(),
            &root_path,
            true,
            false,
            false,
            false,
            None,
            None,
            filter.as_ref(),
        ))
    }

    /// Run all analysis passes
    #[cli(display_with = "display_report")]
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
    ) -> Result<AnalyzeReport, String> {
        let root_path = Self::root_path(root);
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

    /// Run security analysis
    #[server(group = "security")]
    #[cli(display_with = "display_security")]
    pub fn security(
        &self,
        #[param(positional, help = "Target file or directory")] _target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<SecurityReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::security::analyze_security(
            &root_path,
        ))
    }

    /// Show complexity trend over git history
    #[server(group = "code")]
    #[cli(display_with = "display_scalar_trend")]
    pub fn complexity_trend(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_scalar_trend(
            &root_path,
            "avg_complexity",
            snapshots.unwrap_or(10),
            false, // lower complexity is better
            |wt| {
                let report = crate::commands::analyze::complexity::analyze_codebase_complexity(
                    wt,
                    usize::MAX,
                    None,
                    None,
                    &[],
                );
                report.full_stats.map(|s| s.total_avg)
            },
        )
    }

    /// Run function length analysis
    #[server(group = "code")]
    #[cli(display_with = "display_length")]
    #[allow(clippy::too_many_arguments)]
    pub fn length(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of functions to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<LengthReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter =
            Self::build_filter_with_config(&root_path, &config.analyze, "length", &exclude, &only);
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let allowlist = crate::commands::analyze::load_allow_file(&root_path, "length-allow");
        let analysis_root = target
            .as_ref()
            .map(|t| root_path.join(t))
            .unwrap_or_else(|| root_path.clone());
        if analysis_root.is_file() {
            return crate::commands::analyze::length::analyze_file_length(&analysis_root)
                .ok_or_else(|| {
                    format!(
                        "could not analyze '{}' — unsupported file type",
                        analysis_root.display()
                    )
                });
        }
        if !analysis_root.is_dir() {
            return Err(format!(
                "'{}' is not a file or directory",
                analysis_root.display()
            ));
        }
        let mut report = crate::commands::analyze::length::analyze_codebase_length(
            &analysis_root,
            effective_limit,
            filter.as_ref(),
            &allowlist,
        );
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use crate::commands::analyze::length::apply_length_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let sub = analysis_root
                .strip_prefix(&root_path)
                .unwrap_or(std::path::Path::new(""))
                .to_path_buf();
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let wt_root = wt.join(&sub);
                Ok(crate::commands::analyze::length::analyze_codebase_length(
                    &wt_root,
                    usize::MAX,
                    None,
                    &[],
                ))
            })?;
            apply_length_diff(&mut report, &baseline, diff_ref);
        }
        Ok(report)
    }

    /// Show function length trend over git history
    #[server(group = "code")]
    #[cli(display_with = "display_scalar_trend")]
    pub fn length_trend(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_scalar_trend(
            &root_path,
            "avg_length",
            snapshots.unwrap_or(10),
            false, // shorter functions is better
            |wt| {
                let report = crate::commands::analyze::length::analyze_codebase_length(
                    wt,
                    usize::MAX,
                    None,
                    &[],
                );
                report.full_stats.map(|s| s.total_avg)
            },
        )
    }

    /// Analyze documentation coverage
    #[server(group = "repo")]
    #[cli(display_with = "display_doc_coverage")]
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
    ) -> Result<DocCoverageReport, String> {
        let root_path = Self::root_path(root);
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

    /// Change-clusters: connected components of temporally coupled files
    #[server(group = "git")]
    #[cli(display_with = "display_coupling_clusters")]
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
    ) -> Result<CouplingClustersReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut merged_exclude = config.analyze.excludes_for("coupling-clusters");
        merged_exclude.extend(exclude);
        let effective_min = min_commits.unwrap_or_else(|| {
            let total = std::process::Command::new("git")
                .args(["rev-list", "--count", "HEAD"])
                .current_dir(&root_path)
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<usize>()
                        .ok()
                })
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
    }

    /// Analyze cross-repo activity over time
    #[server(group = "git")]
    #[cli(display_with = "display_activity")]
    pub fn activity(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window granularity: month (default) or week")] window: Option<
            crate::commands::analyze::activity::WindowGranularity,
        >,
        #[param(help = "Number of windows to show")] windows: Option<usize>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ActivityReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        crate::commands::analyze::activity::analyze_activity(
            &repos,
            window.unwrap_or_default(),
            windows.unwrap_or(12),
        )
    }

    /// Analyze cross-repo coupling
    #[server(group = "git")]
    #[cli(display_with = "display_repo_coupling")]
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
        crate::commands::analyze::repo_coupling::analyze_repo_coupling(
            &repos,
            window.unwrap_or(24),
            min_windows.unwrap_or(3),
        )
    }

    /// Rank repos by tech debt (churn + complexity + coupling)
    #[server(group = "git")]
    #[cli(display_with = "display_cross_repo_health")]
    pub fn cross_repo_health(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<CrossRepoHealthReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        Ok(crate::commands::analyze::cross_repo_health::analyze_cross_repo_health(&repos))
    }

    /// Find untested public functions ranked by risk
    #[server(group = "test")]
    #[cli(display_with = "display_test_gaps")]
    #[allow(clippy::too_many_arguments)]
    pub async fn test_gaps(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(help = "Show all functions including tested")] all: bool,
        #[param(help = "Only show functions above this risk threshold")] min_risk: Option<f64>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<TestGapsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter = Self::build_filter_with_config(
            &root_path,
            &config.analyze,
            "test-gaps",
            &exclude,
            &only,
        );
        let allowlist = crate::commands::analyze::load_allow_file(&root_path, "test-gaps-allow");
        let effective_limit = match limit.unwrap_or(20) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::test_gaps::analyze_test_gaps(
            &root_path,
            target.as_deref(),
            all,
            min_risk,
            effective_limit,
            filter.as_ref(),
            &allowlist,
        )
        .await)
    }

    /// Show test ratio trend over git history
    #[server(group = "test")]
    #[cli(display_with = "display_scalar_trend")]
    pub fn test_ratio_trend(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_scalar_trend(
            &root_path,
            "overall_test_ratio",
            snapshots.unwrap_or(10),
            true, // higher test ratio is better
            |wt| {
                let report =
                    crate::commands::analyze::test_ratio::analyze_test_ratio(wt, usize::MAX);
                Some(report.overall_ratio)
            },
        )
    }

    /// Show information density trend over git history
    #[server(group = "modules")]
    #[cli(display_with = "display_scalar_trend")]
    pub fn density_trend(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_scalar_trend(
            &root_path,
            "overall_density_score",
            snapshots.unwrap_or(10),
            true, // higher density score is better
            |wt| {
                let report = crate::commands::analyze::density::analyze_density(wt, usize::MAX, 0);
                Some((report.overall_compression_ratio + report.overall_token_uniqueness) / 2.0)
            },
        )
    }

    /// Auto-generated single-page codebase overview
    #[cli(display_with = "display_summary")]
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
    ) -> Result<SummaryReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(5) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::summary::analyze_summary(&root_path, effective_limit).await)
    }

    /// Show structural changes between a base ref and HEAD
    #[server(group = "diff")]
    #[cli(display_with = "display_skeleton_diff")]
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
    ) -> Result<SkeletonDiffReport, String> {
        let root_path = Self::root_path(root);
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
    }

    /// Track health metrics over git history at regular intervals
    #[server(group = "git")]
    #[cli(display_with = "display_trend")]
    pub fn trend(
        &self,
        #[param(short = 'n', help = "Number of historical snapshots (default: 6)")]
        snapshots: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<TrendReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_trend(&root_path, snapshots.unwrap_or(6))
    }

    /// List node kinds and field names for a tree-sitter grammar
    #[server(group = "code")]
    #[cli(display_with = "display_node_types")]
    pub fn node_types(
        &self,
        #[param(positional, help = "Language name (e.g. rust, python, go)")] language: String,
        #[param(help = "Filter types and fields by substring (case-insensitive)")] search: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<NodeTypesReport, String> {
        let root_path = Self::root_path(None);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::syntax::node_types::node_types_for_language(&language, search.as_deref())
    }
}
