//! Rank sub-service for server-less CLI.
//!
//! Hosts all commands that produce an ordered list of items by some metric.

use crate::analyze::complexity::ComplexityReport;
use crate::analyze::function_length::LengthReport;
use crate::analyze::test_gaps::TestGapsReport;
use crate::commands::analyze::budget::LineBudgetReport;
use crate::commands::analyze::call_complexity::CallComplexityReport;
use crate::commands::analyze::ceremony::CeremonyReport;
use crate::commands::analyze::density::DensityReport;
use crate::commands::analyze::files::FileLengthReport;
use crate::commands::analyze::imports::ImportCentralityReport;
use crate::commands::analyze::module_health::ModuleHealthReport;
use crate::commands::analyze::size::SizeReport;
use crate::commands::analyze::surface::SurfaceReport;
use crate::commands::analyze::test_ratio::TestRatioReport;
use crate::commands::analyze::uniqueness::UniquenessReport;
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Rank sub-service: ranked-list commands (ordered by a metric).
pub struct RankService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl RankService {
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

    fn resolve_format(&self, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty =
            !self.compact_raw.get() && (self.pretty_raw.get() || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_complexity(&self, r: &ComplexityReport) -> String {
        self.display_output(r)
    }

    fn display_file_length(&self, r: &FileLengthReport) -> String {
        self.display_output(r)
    }

    fn display_size(&self, r: &SizeReport) -> String {
        self.display_output(r)
    }

    fn display_ceremony(&self, r: &CeremonyReport) -> String {
        self.display_output(r)
    }

    fn display_test_ratio(&self, r: &TestRatioReport) -> String {
        self.display_output(r)
    }

    fn display_budget(&self, r: &LineBudgetReport) -> String {
        self.display_output(r)
    }

    fn display_density(&self, r: &DensityReport) -> String {
        self.display_output(r)
    }

    fn display_uniqueness(&self, r: &UniquenessReport) -> String {
        self.display_output(r)
    }

    fn display_imports(&self, r: &ImportCentralityReport) -> String {
        self.display_output(r)
    }

    fn display_surface(&self, r: &SurfaceReport) -> String {
        self.display_output(r)
    }

    fn display_module_health(&self, r: &ModuleHealthReport) -> String {
        self.display_output(r)
    }

    fn display_call_complexity(&self, r: &CallComplexityReport) -> String {
        self.display_output(r)
    }

    fn display_length(&self, r: &LengthReport) -> String {
        self.display_output(r)
    }

    fn display_test_gaps(&self, r: &TestGapsReport) -> String {
        self.display_output(r)
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

impl server_less::CliGlobals for RankService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "rank",
    description = "Rank files and functions by metrics. Use to find the most complex, longest, or most coupled code.",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
#[server(groups(
    code = "Code quality",
    modules = "Module structure",
    repo = "Repository",
    git = "Git history",
    test = "Testing",
))]
impl RankService {
    /// Rank functions by cyclomatic complexity, worst first.
    ///
    /// Accepts an optional `target` path, a complexity `threshold`, and a `limit` on results.
    /// Use `diff` to compare against a git ref and show deltas. Returns a `ComplexityReport`
    /// with per-function complexity scores, file locations, and optional delta values.
    ///
    /// The `Risk` column bands each function by McCabe cyclomatic complexity:
    /// Low (1-5), Moderate (6-10), High (11-20), Critical (21+).
    #[server(group = "code")]
    #[cli(display_with = "display_complexity")]
    #[allow(clippy::too_many_arguments)]
    pub fn complexity(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 't', help = "Only show functions above this threshold")] threshold: Option<
            usize,
        >,
        #[param(short = 'l', help = "Maximum number of functions to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<ComplexityReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter = Self::build_filter_with_config(
            &root_path,
            &config.analyze,
            "complexity",
            &exclude,
            &only,
        );
        let effective_threshold = threshold.or_else(|| config.analyze.threshold());
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let allowlist = crate::commands::analyze::load_allow_file(&root_path, "complexity-allow");
        let analysis_root = target
            .as_ref()
            .map(|t| root_path.join(t))
            .unwrap_or_else(|| root_path.clone());
        if analysis_root.is_file() {
            return crate::commands::analyze::complexity::analyze_file_complexity(&analysis_root)
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
        let mut report = crate::commands::analyze::complexity::analyze_codebase_complexity(
            &analysis_root,
            effective_limit,
            effective_threshold,
            filter.as_ref(),
            &allowlist,
        );
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::complexity::apply_complexity_diff;
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            let hash = resolve_ref(&root_path, diff_ref)?;
            // Compute relative sub-path of analysis_root within root_path (if any)
            let sub = analysis_root
                .strip_prefix(&root_path)
                .unwrap_or(std::path::Path::new(""))
                .to_path_buf();
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let wt_root = wt.join(&sub);
                Ok(
                    crate::commands::analyze::complexity::analyze_codebase_complexity(
                        &wt_root,
                        usize::MAX, // get all functions for accurate baseline matching
                        None,
                        None,
                        &[],
                    ),
                )
            })?;
            apply_complexity_diff(&mut report, &baseline, diff_ref);
        }
        Ok(report)
    }

    /// Rank source files by line count, longest first.
    ///
    /// Returns a `FileLengthReport` with file paths and line counts. Supports `exclude`
    /// globs and an optional `diff` ref to show how file lengths have changed.
    #[server(group = "repo")]
    #[cli(display_with = "display_file_length")]
    pub fn files(
        &self,
        #[param(short = 'l', help = "Maximum number of files to show (0=no limit)")] limit: Option<
            usize,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<FileLengthReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut merged_exclude = config.analyze.excludes_for("files");
        merged_exclude.extend(exclude);
        let mut report = crate::commands::analyze::files::analyze_files(
            &root_path,
            limit.unwrap_or(20),
            &merged_exclude,
        );
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::files::analyze_files(
                    wt,
                    usize::MAX,
                    &merged_exclude,
                ))
            })?;
            compute_ranked_diff(&mut report.files, &baseline.files);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Show hierarchical lines-of-code breakdown (ncdu-style tree view).
    ///
    /// Recursively aggregates line counts by directory, returning a `SizeReport` with
    /// nested entries. The largest directories appear first at each level, mirroring
    /// how ncdu presents disk usage.
    #[server(group = "modules")]
    #[cli(display_with = "display_size")]
    pub fn size(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
    ) -> Result<SizeReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut merged_exclude = config.analyze.excludes_for("size");
        merged_exclude.extend(exclude);
        Ok(crate::commands::analyze::size::analyze_size(
            &root_path,
            &merged_exclude,
        ))
    }

    /// Rank files by boilerplate ratio: fraction of callables that are trait/interface implementations.
    ///
    /// High ceremony scores indicate files dominated by mechanical delegation rather than
    /// business logic. Returns a `CeremonyReport` with per-file ratios and an optional
    /// delta when `diff` is provided.
    #[server(group = "code")]
    #[cli(display_with = "display_ceremony")]
    pub fn ceremony(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of files to show (0=no limit)")] limit: Option<
            usize,
        >,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<CeremonyReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let mut report =
            crate::commands::analyze::ceremony::analyze_ceremony(&root_path, limit.unwrap_or(15));
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::ceremony::analyze_ceremony(
                    wt,
                    usize::MAX,
                ))
            })?;
            compute_ranked_diff(&mut report.top_files, &baseline.top_files);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by test-to-implementation line ratio.
    ///
    /// Classifies files as test or implementation by naming convention and groups them
    /// by module. Returns a `TestRatioReport` with per-module ratios and an overall score.
    /// Modules with the lowest ratio appear first.
    #[server(group = "test")]
    #[cli(display_with = "display_test_ratio")]
    pub fn test_ratio(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<TestRatioReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let mut report =
            crate::commands::analyze::test_ratio::analyze_test_ratio(&root_path, effective_limit);
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::test_ratio::analyze_test_ratio(
                    wt,
                    usize::MAX,
                ))
            })?;
            compute_ranked_diff(&mut report.entries, &baseline.entries);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Find the longest functions in the codebase, ranked by line count.
    ///
    /// Accepts an optional `target` path, a `limit` on results, an `exclude` glob list,
    /// and a `diff` ref to compare against. Returns a `LengthReport` with per-function
    /// entries including file, line range, and optional delta from the diff ref.
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
    ) -> Result<LengthReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
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

    /// Identify public functions that lack test coverage, ranked by risk score.
    ///
    /// Also known as: untested code, coverage gaps, dead tests, orphaned functions, missing
    /// test coverage. Finds public callables with no test references and ranks them by risk.
    ///
    /// Uses the facts index to find callables with no test references, then ranks them
    /// by a risk heuristic (complexity × call-site count). `min_risk` filters out low-risk
    /// entries. Returns a `TestGapsReport` with per-function risk scores and locations.
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
    ) -> Result<TestGapsReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
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

    /// Break down line counts by purpose: business logic, tests, docs, config, and generated code.
    ///
    /// Classifies every line in the codebase into a budget category and ranks files by the
    /// categories you care about most. Returns a `LineBudgetReport` with per-file breakdowns
    /// and totals across the whole project.
    ///
    /// Renamed from `rank budget` to `rank purposes` to free the `budget` word for the
    /// `normalize-budget` crate verb.
    #[server(group = "test")]
    #[cli(name = "purposes", display_with = "display_budget")]
    pub fn budget(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<LineBudgetReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let mut report =
            crate::commands::analyze::budget::analyze_budget(&root_path, effective_limit);
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::budget::analyze_budget(
                    wt,
                    usize::MAX,
                ))
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by information density: compression ratio combined with token uniqueness.
    ///
    /// Modules with high density pack more distinct concepts per line. Low-density modules
    /// may have excessive boilerplate or copy-paste.
    ///
    /// Metrics:
    /// - **Compression ratio**: `compressed_bytes / original_bytes` (gzip) — lower = more
    ///   repetitive structure (boilerplate, templated code).
    /// - **Token uniqueness**: `unique_tokens / total_tokens` — lower = more repeated
    ///   vocabulary (copy-paste, uniform naming patterns).
    /// - **Density score**: `(compression_ratio + token_uniqueness) / 2` — combined score;
    ///   lower = more repetitive overall. Modules are ranked lowest-first.
    ///
    /// Returns a `DensityReport` with per-module scores and the worst individual files.
    #[server(group = "modules")]
    #[cli(display_with = "display_density")]
    pub fn density(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(short = 'w', help = "Number of worst files to show (default: 10)")] worst: Option<
            usize,
        >,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<DensityReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let module_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let mut report = crate::commands::analyze::density::analyze_density(
            &root_path,
            module_limit,
            worst.unwrap_or(10),
        );
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::density::analyze_density(
                    wt,
                    usize::MAX,
                    0,
                ))
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by code uniqueness: fraction of functions with no structural near-twin.
    ///
    /// Also known as: DRY violations, code reuse opportunities, copy-paste hotspots.
    /// Modules with low uniqueness scores have many similar functions that are candidates
    /// for extraction or consolidation.
    ///
    /// Uses MinHash similarity to find near-duplicate function bodies. Returns a
    /// `UniquenessReport` with per-module scores.
    #[server(group = "code")]
    #[cli(display_with = "display_uniqueness")]
    #[allow(clippy::too_many_arguments)]
    pub fn uniqueness(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Similarity threshold 0.0–1.0 (default: 0.80)")] similarity: Option<f64>,
        #[param(help = "Minimum function line count to include (default: 5)")] min_lines: Option<
            usize,
        >,
        #[param(help = "Match on control-flow skeleton only")] skeleton: bool,
        #[param(help = "Include groups where all functions share the same name")]
        include_trait_impls: bool,
        #[param(help = "Number of top clusters to show (default: 10)")] clusters: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<UniquenessReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let module_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let config = crate::config::NormalizeConfig::load(&root_path);
        let filter = Self::build_filter_with_config(
            &root_path,
            &config.analyze,
            "uniqueness",
            &exclude,
            &only,
        );
        let sim = similarity.unwrap_or(0.80);
        let min = min_lines.unwrap_or(5);
        let clust = clusters.unwrap_or(10);
        let mut report = crate::commands::analyze::uniqueness::analyze_uniqueness(
            &root_path,
            sim,
            min,
            skeleton,
            include_trait_impls,
            module_limit,
            clust,
            filter.as_ref(),
        );
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                Ok(crate::commands::analyze::uniqueness::analyze_uniqueness(
                    wt,
                    sim,
                    min,
                    skeleton,
                    include_trait_impls,
                    usize::MAX,
                    0,
                    None,
                ))
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by import fan-in: how many other modules import each module.
    ///
    /// Also known as: most imported modules, architectural hubs, core dependencies, widely-used
    /// modules, popular APIs. High fan-in modules are the ones with the highest blast radius —
    /// changes to them ripple widely across the codebase.
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns an `ImportCentralityReport`
    /// with per-module fan-in counts and the most-imported symbols.
    #[server(group = "modules")]
    #[cli(display_with = "display_imports")]
    pub async fn imports(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show only internal (crate-local) modules")] internal: bool,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<ImportCentralityReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let mut report = crate::commands::analyze::imports::analyze_import_centrality(
            &root_path,
            effective_limit,
            internal,
        )
        .await?;
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let handle = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    handle.block_on(
                        crate::commands::analyze::imports::analyze_import_centrality(
                            wt,
                            usize::MAX,
                            internal,
                        ),
                    )
                })
            })?;
            compute_ranked_diff(&mut report.entries, &baseline.entries);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by API surface area: public symbol count, public ratio, and constraint score.
    ///
    /// Also known as: public API size, interface bloat, over-exposed modules. Modules with large
    /// surfaces are harder to evolve without breaking callers (high blast radius).
    ///
    /// The constraint score is `public_symbols × fan_in` — modules with many public symbols that
    /// are widely imported are the hardest to change safely. A module with 50 public symbols and
    /// 20 importers scores 1000; a module with 5 public symbols and 2 importers scores 10.
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns a `SurfaceReport`
    /// with per-module rankings sorted by constraint score (highest first).
    #[server(group = "modules")]
    #[cli(display_with = "display_surface")]
    pub async fn surface(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<SurfaceReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let idx = crate::index::ensure_ready(&root_path).await?;
        let mut report = crate::commands::analyze::surface::analyze_surface(&idx, effective_limit)
            .await
            .map_err(|e| format!("Surface analysis failed: {}", e))?;
        if let Some(ref diff_ref) = diff {
            use crate::commands::analyze::git_history::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let handle = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        let wt_idx = crate::index::ensure_ready(wt).await?;
                        crate::commands::analyze::surface::analyze_surface(&wt_idx, usize::MAX)
                            .await
                            .map_err(|e| format!("Baseline surface failed: {}", e))
                    })
                })
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Score each module on a composite health metric: test ratio × uniqueness × density.
    ///
    /// Combines three orthogonal quality signals into a single score per module. Modules
    /// with the lowest composite score appear first and are the highest-priority candidates
    /// for improvement. Returns a `ModuleHealthReport`.
    #[server(group = "modules")]
    #[cli(display_with = "display_module_health")]
    pub fn module_health(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Minimum lines for a module to be included (default: 100)")]
        min_lines: Option<usize>,
    ) -> Result<ModuleHealthReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = match limit.unwrap_or(0) {
            0 => usize::MAX,
            n => n,
        };
        Ok(
            crate::commands::analyze::module_health::analyze_module_health(
                &root_path,
                effective_limit,
                min_lines.unwrap_or(100),
            ),
        )
    }

    /// Rank functions by effective complexity: cyclomatic complexity summed over the full call graph.
    ///
    /// Also known as: transitive complexity, call hierarchy depth, deep complexity, total
    /// cognitive load. Reveals functions that look simple locally but trigger large complex
    /// call trees — a common source of hard-to-test and hard-to-debug code.
    ///
    /// Uses BFS on the call graph (requires facts index) to compute the total complexity
    /// reachable from each entry point. High scores indicate functions that are simple
    /// locally but call many complex sub-functions. Returns a `CallComplexityReport`.
    ///
    /// **Amplification** (`Top Amplified` table): `reachable_cc / local_cc`. A high ratio
    /// means the function is a thin dispatcher into complex territory — simple locally but
    /// triggering many complex sub-functions. **Reachable CC** is the sum of cyclomatic
    /// complexity over all functions reachable via BFS from this entry point.
    /// **Reachable Count** is the number of distinct functions reachable.
    #[server(group = "code")]
    #[cli(display_with = "display_call_complexity")]
    pub fn call_complexity(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum functions to show per list (default: 20)")]
        limit: Option<usize>,
        #[param(short = 'm', help = "Maximum number of modules to show (0=no limit)")]
        module_limit: Option<usize>,
    ) -> Result<CallComplexityReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
        let effective_limit = limit.unwrap_or(20);
        let effective_module_limit = match module_limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        Ok(
            crate::commands::analyze::call_complexity::analyze_call_complexity(
                &root_path,
                effective_limit,
                effective_module_limit,
            ),
        )
    }
}
