//! Overview sub-service for server-less CLI.
//!
//! Thin main-crate composition verb over the analyze dashboards. `overview`
//! aggregates health/quality signals (formerly `analyze health`), `overview
//! --full` runs all passes (formerly `analyze all`), `overview summary` renders
//! the single-page codebase overview (formerly `analyze summary`), and
//! `overview cross-repo-health` ranks repositories by composite tech-debt.
//!
//! These are cross-cutting compositions with no owning compute crate, so per
//! CLAUDE.md they stay main-crate-resident. The underlying compute lives in
//! `crate::commands::analyze::*`; this service only composes and formats it.

use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
use crate::commands::analyze::report::AnalyzeReport;
use crate::commands::analyze::summary::SummaryReport;
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Errors returned by the overview service.
#[derive(Debug, thiserror::Error)]
pub enum OverviewError {
    /// An error with a descriptive message (forwarded as-is).
    #[error("{0}")]
    Message(String),
}

impl From<String> for OverviewError {
    fn from(s: String) -> Self {
        OverviewError::Message(s)
    }
}

/// Overview sub-service (health dashboard, summary, cross-repo health).
pub struct OverviewService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl OverviewService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
        }
    }

    fn root_path(root: Option<String>) -> Result<PathBuf, OverviewError> {
        root.map(PathBuf::from).map_or_else(
            || {
                std::env::current_dir().map_err(|e| {
                    OverviewError::Message(format!("failed to get working directory: {e}"))
                })
            },
            Ok,
        )
    }

    fn resolve_format(&self, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty =
            !self.compact_raw.get() && (self.pretty_raw.get() || config.pretty.enabled());
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

    /// Build a filter with merged excludes: config global + per-subcommand + CLI args.
    fn build_filter(
        root: &std::path::Path,
        subcommand: &str,
        cli_exclude: &[String],
        only: &[String],
    ) -> Option<crate::filter::Filter> {
        let config = crate::config::NormalizeConfig::load(root);
        let mut excludes = config.analyze.excludes_for(subcommand);
        excludes.extend(cli_exclude.iter().cloned());
        if excludes.is_empty() && only.is_empty() {
            None
        } else {
            crate::commands::build_filter(root, &excludes, only)
        }
    }
}

impl server_less::CliGlobals for OverviewService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "overview",
    description = "Codebase dashboards: health, full analysis, single-page summary, cross-repo health.",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl OverviewService {
    /// Codebase health dashboard: file counts, complexity stats, large-file warnings.
    ///
    /// The default overview. Pass `--full` to run every analysis pass (health +
    /// complexity + length + security), the equivalent of the former `analyze all`.
    ///
    /// Examples:
    ///   normalize overview                       # health dashboard
    ///   normalize overview --full                # run all analysis passes
    ///   normalize overview src/                  # scope to a subtree
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
        #[param(help = "Run all analysis passes (health + complexity + length + security)")]
        full: bool,
    ) -> Result<AnalyzeReport, OverviewError> {
        let root_path = Self::root_path(root)?;
        if let Some(ref t) = target {
            let candidate = root_path.join(t);
            if !candidate.exists() && !t.contains('*') && !t.contains('?') && !t.contains('[') {
                return Err(OverviewError::Message(format!("path not found: {t}")));
            }
        }
        self.resolve_format(&root_path);
        let subcommand = if full { "all" } else { "health" };
        let filter = Self::build_filter(&root_path, subcommand, &exclude, &only);
        let mut report = crate::commands::analyze::report::analyze(
            target.as_deref(),
            &root_path,
            true, // health
            full, // complexity
            full, // length
            full, // security
            None,
            None,
            filter.as_ref(),
        );
        // Cap large_files to avoid bloated JSON output for agents (health path only).
        let cap = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        if let Some(ref mut health) = report.health {
            health.large_files.truncate(cap);
        }
        Ok(report)
    }

    /// Auto-generated single-page codebase overview.
    ///
    /// Example:
    ///   normalize overview summary
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
    ) -> Result<SummaryReport, OverviewError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(5) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::summary::analyze_summary(&root_path, effective_limit).await)
    }

    /// Rank repositories by composite tech-debt score (churn × complexity × coupling).
    ///
    /// Discovers git repos under `repos_dir` and computes a health score for each by
    /// combining churn rate, average cyclomatic complexity, and temporal coupling density.
    /// Returns a `CrossRepoHealthReport` with repos ranked worst-first.
    ///
    /// Example:
    ///   normalize overview cross-repo-health ~/src
    #[cli(name = "cross-repo-health", display_with = "display_output")]
    pub fn cross_repo_health(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<CrossRepoHealthReport, OverviewError> {
        let repos = crate::multi_repo::discover_repos_depth(
            &PathBuf::from(&repos_dir),
            repos_depth.unwrap_or(1),
        )
        .map_err(OverviewError::Message)?;
        self.resolve_format(&std::env::current_dir().unwrap_or_default());
        Ok(crate::commands::analyze::cross_repo_health::analyze_cross_repo_health(&repos))
    }
}
