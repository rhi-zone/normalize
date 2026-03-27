//! Trend sub-service for server-less CLI.
//!
//! Hosts all commands that track health metrics over git history at regular intervals.
//! The default command (`normalize trend`) runs all metrics; subcommands target individual ones.

use crate::commands::analyze::trend::{ScalarTrendReport, TrendReport};
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Trend sub-service: time-series commands tracking health metrics over git history.
pub struct TrendService {
    pretty: Cell<bool>,
}

impl TrendService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
        }
    }

    fn root_path(root: Option<String>) -> Result<PathBuf, String> {
        root.map(PathBuf::from).map_or_else(
            || std::env::current_dir().map_err(|e| format!("failed to get working directory: {e}")),
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
}

#[cli(
    name = "trend",
    description = "Track health metrics (complexity, length, test ratio, density) over git history",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl TrendService {
    /// Track multiple health metrics (complexity, length, test ratio, density) over git history.
    ///
    /// Walks `snapshots` evenly-spaced commits on the current branch and collects a composite
    /// set of metrics at each point. Returns a `TrendReport` with per-snapshot values for
    /// all tracked metrics, enabling holistic codebase trend analysis.
    #[cli(display_with = "display_output")]
    pub fn multi(
        &self,
        #[param(short = 'n', help = "Number of historical snapshots (default: 6)")]
        snapshots: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<TrendReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::trend::analyze_trend(&root_path, snapshots.unwrap_or(6))
    }

    /// Show how average cyclomatic complexity has changed over git history.
    ///
    /// Walks `snapshots` evenly-spaced commits on the current branch, computing average
    /// complexity at each point. Returns a `ScalarTrendReport` suitable for plotting
    /// or diffing. Lower values indicate improvement.
    #[cli(display_with = "display_output")]
    pub fn complexity(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root)?;
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

    /// Show how average function length has changed over git history.
    ///
    /// Walks `snapshots` evenly-spaced commits, computing average function line count at
    /// each point. Returns a `ScalarTrendReport`. Lower values indicate improvement.
    #[cli(display_with = "display_output")]
    pub fn length(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root)?;
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

    /// Show how information density has changed over git history.
    ///
    /// Walks `snapshots` evenly-spaced commits and computes a composite density score
    /// (compression ratio + token uniqueness) at each point. Returns a `ScalarTrendReport`.
    /// Higher values indicate denser, more information-rich code over time.
    #[cli(display_with = "display_output")]
    pub fn density(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root)?;
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

    /// Show how the test-to-code ratio has changed over git history.
    ///
    /// Walks `snapshots` evenly-spaced commits and computes the fraction of files that
    /// are test files at each point. Returns a `ScalarTrendReport`. Higher values indicate
    /// better test coverage over time.
    #[cli(name = "test-ratio", display_with = "display_output")]
    pub fn test_ratio(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'n', help = "Number of snapshots to collect (default: 10)")]
        snapshots: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<ScalarTrendReport, String> {
        let root_path = Self::root_path(root)?;
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
}
