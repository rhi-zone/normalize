//! Analyze sub-service for server-less CLI.

use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::report::SecurityReport;
use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
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

/// Analyze sub-service (health, complexity, security, duplicates, docs).
pub struct AnalyzeService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl AnalyzeService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
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

impl server_less::CliGlobals for AnalyzeService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "analyze",
    description = "Assess codebase quality. Use for health checks, finding duplicates, security scanning, and architecture analysis.",
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
    ) -> Result<SkeletonDiffReport, AnalyzeError> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(&root_path);
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
