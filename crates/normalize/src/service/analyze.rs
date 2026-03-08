//! Analyze sub-service for server-less CLI.

use crate::analyze::complexity::ComplexityReport;
use crate::analyze::function_length::LengthReport;
use crate::analyze::test_gaps::TestGapsReport;
use crate::commands::analyze::activity::ActivityReport;
use crate::commands::analyze::architecture::ArchitectureReport;
use crate::commands::analyze::budget::BudgetReport;
use crate::commands::analyze::call_complexity::CallComplexityReport;
use crate::commands::analyze::call_graph::CallEntry;
use crate::commands::analyze::ceremony::CeremonyReport;
use crate::commands::analyze::contributors::ContributorsReport;
use crate::commands::analyze::coupling::CouplingReport;
use crate::commands::analyze::coupling_clusters::CouplingClustersReport;
use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
use crate::commands::analyze::density::DensityReport;
use crate::commands::analyze::depth_map::DepthMapReport;
use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::duplicates::{
    DuplicateBlocksConfig, DuplicateFunctionsConfig, DuplicateTypesReport, SimilarBlocksConfig,
    SimilarFunctionsConfig,
};
use crate::commands::analyze::duplicates_views::{DuplicateMode, DuplicateScope, DuplicatesReport};
use crate::commands::analyze::files::FileLengthReport;
use crate::commands::analyze::fragments::{FragmentScope, FragmentsReport};
use crate::commands::analyze::graph::{GraphReport, GraphTarget};
use crate::commands::analyze::hotspots::HotspotsReport;
use crate::commands::analyze::impact::ImpactReport;
use crate::commands::analyze::imports::ImportCentralityReport;
use crate::commands::analyze::layering::LayeringReport;
use crate::commands::analyze::module_health::ModuleHealthReport;
use crate::commands::analyze::ownership::{OwnershipRepoEntry, OwnershipReport};
use crate::commands::analyze::provenance::ProvenanceReport;
use crate::commands::analyze::repo_coupling::RepoCouplingReport;
use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
use crate::commands::analyze::size::SizeReport;
use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
use crate::commands::analyze::summary::SummaryReport;
use crate::commands::analyze::surface::SurfaceReport;
use crate::commands::analyze::test_ratio::TestRatioReport;
use crate::commands::analyze::trend::TrendReport;
use crate::commands::analyze::uniqueness::UniquenessReport;
use crate::output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
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

    fn display_check(&self, r: &DiagnosticsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_call_graph(&self, entries: &[CallEntry]) -> String {
        entries
            .iter()
            .map(|e| format!("  {}:{}:{}", e.file, e.line, e.symbol))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn display_trace(&self, text: &str) -> String {
        text.to_string()
    }

    fn display_architecture(&self, r: &ArchitectureReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_impact(&self, r: &ImpactReport) -> String {
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

    fn display_complexity(&self, r: &ComplexityReport) -> String {
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

    fn display_file_length(&self, r: &FileLengthReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_size(&self, r: &SizeReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_ceremony(&self, r: &CeremonyReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_ownership(&self, r: &OwnershipReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_contributors(&self, r: &ContributorsReport) -> String {
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

    fn display_duplicates(&self, r: &DuplicatesReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_dup_types(&self, r: &DuplicateTypesReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_depth_map(&self, r: &DepthMapReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_graph(&self, r: &GraphReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_surface(&self, r: &SurfaceReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_layering(&self, r: &LayeringReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_provenance(&self, r: &ProvenanceReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_density(&self, r: &DensityReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_uniqueness(&self, r: &UniquenessReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_call_complexity(&self, r: &CallComplexityReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_module_health(&self, r: &ModuleHealthReport) -> String {
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

    fn display_imports(&self, r: &ImportCentralityReport) -> String {
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

    fn display_fragments(&self, r: &FragmentsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_test_ratio(&self, r: &TestRatioReport) -> String {
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

    fn display_budget(&self, r: &BudgetReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_coupling(&self, r: &CouplingReport) -> String {
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

    fn display_hotspots(&self, r: &HotspotsReport) -> String {
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
}

#[cli(
    name = "analyze",
    about = "Analyze codebase (health, complexity, security, duplicates, docs)"
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
    /// Check documentation health: broken refs, stale docs, missing examples, and SUMMARY.md freshness.
    #[server(group = "repo")]
    #[cli(display_with = "display_check")]
    #[allow(clippy::too_many_arguments)]
    pub async fn check(
        &self,
        #[param(help = "Check broken documentation references")] refs: bool,
        #[param(help = "Check for stale documentation")] stale: bool,
        #[param(help = "Check for missing example references")] examples: bool,
        #[param(help = "Check for missing or stale SUMMARY.md files")] summary: bool,
        #[param(
            help = "Staleness threshold: flag when (commits_since_update + has_uncommitted) > N (default: 10)"
        )]
        summary_threshold: Option<usize>,
        #[param(help = "Exit 0 even when error-severity issues are found")] no_fail: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<DiagnosticsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let run_all = !refs && !stale && !examples && !summary;

        let mut report = DiagnosticsReport::new();

        if run_all || refs {
            let refs_report = normalize_native_rules::build_check_refs_report(&root_path).await?;
            report.merge(refs_report.into());
        }

        if run_all || stale {
            let stale_report = normalize_native_rules::build_stale_docs_report(&root_path);
            report.merge(stale_report.into());
        }

        if run_all || examples {
            let examples_report = normalize_native_rules::build_check_examples_report(&root_path);
            report.merge(examples_report.into());
        }

        if run_all || summary {
            let threshold = summary_threshold.unwrap_or(10);
            let summary_report =
                normalize_native_rules::build_stale_summary_report(&root_path, threshold);
            report.merge(summary_report.into());
        }

        // Apply per-rule severity/enabled overrides from normalize.toml to native check issues.
        // This allows e.g. [analyze.rules."stale-summary"] severity = "error" to enforce SUMMARY.md freshness.
        normalize_rules::apply_native_rules_config(&mut report, &config.analyze.rules);

        report.sort();

        let error_count = report.count_by_severity(normalize_output::diagnostics::Severity::Error);
        if !no_fail && error_count > 0 {
            let detail = if self.pretty.get() {
                report.format_pretty()
            } else {
                report.format_text()
            };
            return Err(format!("{detail}\n{error_count} error(s) found"));
        }

        if !report.issues.is_empty() && !self.pretty.get() {
            // Determine which check(s) were run to build a precise suggestion.
            let flag = if run_all {
                "normalize analyze check".to_string()
            } else {
                let flags: Vec<&str> = [
                    refs.then_some("--refs"),
                    stale.then_some("--stale"),
                    examples.then_some("--examples"),
                    summary.then_some("--summary"),
                ]
                .into_iter()
                .flatten()
                .collect();
                format!("normalize analyze check {}", flags.join(" "))
            };
            report
                .hints
                .push(format!("Run `{flag} --pretty` for a detailed view"));
        }

        Ok(report)
    }

    /// Show callers and/or callees of a symbol
    #[server(group = "graph")]
    #[cli(display_with = "display_call_graph")]
    pub async fn call_graph(
        &self,
        #[param(positional, help = "Symbol to look up (or file#symbol)")] target: String,
        #[param(help = "Show callers")] callers: bool,
        #[param(help = "Show callees")] callees: bool,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<CallEntry>, String> {
        let root_path = Self::root_path(root);
        let show_callers = callers || !callees;
        crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &target,
            show_callers,
            callees,
            case_insensitive,
        )
        .await
    }

    /// Trace value provenance for a symbol
    #[server(group = "graph")]
    #[cli(display_with = "display_trace")]
    pub fn trace(
        &self,
        #[param(positional, help = "Symbol to trace (file/symbol or symbol name)")] symbol: String,
        #[param(short = 't', help = "Target file containing the symbol")] target: Option<String>,
        #[param(short = 'd', help = "Maximum depth")] max_depth: Option<usize>,
        #[param(help = "Recursively trace called functions")] recursive: bool,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<String, String> {
        let root_path = Self::root_path(root);
        crate::commands::analyze::trace::build_trace_text(
            &symbol,
            target.as_deref(),
            &root_path,
            max_depth.unwrap_or(50),
            recursive,
            case_insensitive,
        )
    }

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

    /// What-if impact analysis: reverse-dependency closure + blast radius
    #[server(group = "graph")]
    #[cli(display_with = "display_impact")]
    pub async fn impact(
        &self,
        #[param(positional, help = "Target file to analyze impact for")] target: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<ImpactReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let idx = crate::index::ensure_ready(&root_path).await?;
        crate::commands::analyze::impact::analyze_impact(&idx, &target)
            .await
            .map_err(|e| format!("Impact analysis failed: {}", e))
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
        let filter = Self::build_filter(&root_path, &exclude, &only);
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
        let filter = Self::build_filter(&root_path, &exclude, &only);
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

    /// Run complexity analysis
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
        pretty: bool,
        compact: bool,
    ) -> Result<ComplexityReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        let config = crate::config::NormalizeConfig::load(&root_path);
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
        Ok(
            crate::commands::analyze::complexity::analyze_codebase_complexity(
                &analysis_root,
                effective_limit,
                effective_threshold,
                filter.as_ref(),
                &allowlist,
            ),
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
        pretty: bool,
        compact: bool,
    ) -> Result<LengthReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let allowlist = crate::commands::analyze::load_allow_file(&root_path, "length-allow");
        let analysis_root = target
            .as_ref()
            .map(|t| root_path.join(t))
            .unwrap_or_else(|| root_path.clone());
        Ok(crate::commands::analyze::length::analyze_codebase_length(
            &analysis_root,
            effective_limit,
            filter.as_ref(),
            &allowlist,
        ))
    }

    /// Analyze documentation coverage
    #[server(group = "repo")]
    #[cli(display_with = "display_doc_coverage")]
    pub fn docs(
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
        let filter = Self::build_filter(&root_path, &exclude, &only);
        let config = crate::config::NormalizeConfig::load(&root_path);
        Ok(crate::commands::analyze::docs::analyze_docs(
            &root_path,
            limit.unwrap_or(10),
            config.analyze.exclude_interface_impls(),
            filter.as_ref(),
        ))
    }

    /// Show longest files in codebase
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
    ) -> Result<FileLengthReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::files::analyze_files(
            &root_path,
            limit.unwrap_or(20),
            &exclude,
        ))
    }

    /// Show hierarchical LOC breakdown (ncdu-style)
    #[server(group = "modules")]
    #[cli(display_with = "display_size")]
    pub fn size(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
    ) -> Result<SizeReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::size::analyze_size(
            &root_path, &exclude,
        ))
    }

    /// Show ceremony ratio: fraction of callables that are trait/interface boilerplate
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
    ) -> Result<CeremonyReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::ceremony::analyze_ceremony(
            &root_path,
            limit.unwrap_or(15),
        ))
    }

    /// Temporal coupling: file pairs that change together in git history
    #[server(group = "git")]
    #[cli(display_with = "display_coupling")]
    pub fn coupling(
        &self,
        #[param(help = "Minimum shared commits for coupling edges")] min_commits: Option<usize>,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CouplingReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let min = min_commits.unwrap_or(3);
        let lim = limit.unwrap_or(20);
        crate::commands::analyze::coupling::analyze_coupling(&root_path, min, lim, &exclude)
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
            &exclude,
            &only,
        )
    }

    /// Churn × complexity hotspots: files ranked by change frequency and complexity
    #[server(group = "git")]
    #[cli(display_with = "display_hotspots")]
    pub fn hotspots(
        &self,
        #[param(help = "Weight recent changes higher (exponential decay)")] recency: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<HotspotsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut excludes = config.analyze.hotspots_exclude.clone();
        excludes.extend(crate::commands::analyze::load_allow_file(
            &root_path,
            "hotspots-allow",
        ));
        crate::commands::analyze::hotspots::analyze_hotspots(&root_path, &excludes, recency)
    }

    /// Show per-file ownership concentration from git blame
    #[server(group = "git")]
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
        #[param(help = "Run across all git repos under DIR")] repos: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<OwnershipReport, String> {
        let root_path = Self::root_path(root);
        let lim = limit.unwrap_or(20);
        if let Some(repos_dir) = repos {
            let repo_paths = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
            let entries: Vec<OwnershipRepoEntry> = repo_paths
                .into_iter()
                .map(|repo_path| {
                    let name = repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    match crate::commands::analyze::ownership::analyze_ownership(
                        &repo_path, lim, &exclude,
                    ) {
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
            });
        }
        crate::commands::analyze::ownership::analyze_ownership(&root_path, lim, &exclude)
    }

    /// Analyze contributors across repos
    #[server(group = "git")]
    #[cli(display_with = "display_contributors")]
    pub fn contributors(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ContributorsReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        crate::commands::analyze::contributors::analyze_contributors(&repos)
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
    #[server(group = "graph")]
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
    #[cli(display_with = "display_cross_repo_health")]
    pub fn cross_repo_health(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<CrossRepoHealthReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        Ok(crate::commands::analyze::cross_repo_health::analyze_cross_repo_health(&repos))
    }

    /// Detect duplicate/similar code (functions or blocks)
    ///
    /// Modes: exact (default), similar (fuzzy MinHash), clusters (connected components).
    #[server(group = "code")]
    #[cli(display_with = "display_duplicates")]
    #[allow(clippy::too_many_arguments)]
    pub fn duplicates(
        &self,
        #[param(help = "Scope: functions (default) or blocks")] scope: Option<DuplicateScope>,
        #[param(help = "Detection mode: exact (default), similar (fuzzy), or clusters")]
        mode: Option<DuplicateMode>,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing")] elide_literals: bool,
        #[param(help = "Show source code for matches")] show_source: bool,
        #[param(help = "Minimum lines to be considered")] min_lines: Option<usize>,
        #[param(help = "Include groups where all items share the same name")]
        include_trait_impls: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
        #[param(help = "Minimum similarity threshold (0.0-1.0, similar/clusters mode)")]
        similarity: Option<f64>,
        #[param(help = "Match on control-flow structure (similar/clusters mode)")] skeleton: bool,
        #[param(help = "Scan across all git repos under DIR (functions scope only)")] repos: Option<
            String,
        >,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
        #[param(help = "Skip function/method nodes (blocks scope only)")] skip_functions: bool,
        #[param(
            short = 'l',
            help = "Maximum number of results to show (0=no limit, clusters mode)"
        )]
        limit: Option<usize>,
    ) -> Result<DuplicatesReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let scope = scope.unwrap_or(DuplicateScope::Functions);
        let mode = mode.unwrap_or(DuplicateMode::Exact);
        let filter = Self::build_filter(&root_path, &exclude, &only);

        match (mode, scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(
                    crate::commands::analyze::duplicates::build_duplicate_functions_report(
                        DuplicateFunctionsConfig {
                            roots: &roots,
                            elide_identifiers,
                            elide_literals,
                            show_source,
                            min_lines: min_lines.unwrap_or(1),
                            include_trait_impls,
                            filter: filter.as_ref(),
                        },
                    ),
                )
            }
            (DuplicateMode::Exact, DuplicateScope::Blocks) => Ok(
                crate::commands::analyze::duplicates::build_duplicate_blocks_report(
                    DuplicateBlocksConfig {
                        root: &root_path,
                        min_lines: min_lines.unwrap_or(5),
                        elide_identifiers,
                        elide_literals,
                        skip_functions,
                        show_source,
                        allow: None,
                        reason: None,
                        filter: filter.as_ref(),
                    },
                ),
            ),
            (DuplicateMode::Similar, DuplicateScope::Functions) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(
                    crate::commands::analyze::duplicates::build_similar_functions_report(
                        SimilarFunctionsConfig {
                            roots: &roots,
                            min_lines: min_lines.unwrap_or(10),
                            similarity: similarity.unwrap_or(0.85),
                            elide_identifiers,
                            elide_literals,
                            skeleton,
                            show_source,
                            include_trait_impls,
                            allow: None,
                            reason: None,
                            filter: filter.as_ref(),
                        },
                    ),
                )
            }
            (DuplicateMode::Similar, DuplicateScope::Blocks) => Ok(
                crate::commands::analyze::duplicates::build_similar_blocks_report(
                    SimilarBlocksConfig {
                        root: &root_path,
                        min_lines: min_lines.unwrap_or(10),
                        similarity: similarity.unwrap_or(0.85),
                        elide_identifiers,
                        elide_literals,
                        skeleton,
                        show_source,
                        include_trait_impls,
                        allow: None,
                        reason: None,
                        filter: filter.as_ref(),
                    },
                ),
            ),
            (DuplicateMode::Clusters, _) => {
                let roots: Vec<PathBuf> = if let Some(repos_dir) = repos {
                    discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
                } else {
                    vec![root_path.clone()]
                };
                Ok(
                    crate::commands::analyze::clusters::build_clusters_report_multi(
                        &roots,
                        min_lines.unwrap_or(10),
                        similarity.unwrap_or(0.85),
                        elide_identifiers,
                        skeleton,
                        include_trait_impls,
                        limit.unwrap_or(20),
                        filter.as_ref(),
                    ),
                )
            }
        }
    }

    /// Detect duplicate type definitions
    #[server(group = "code")]
    #[cli(display_with = "display_dup_types")]
    pub fn duplicate_types(
        &self,
        #[param(positional, help = "Target directory to scan")] target: Option<String>,
        #[param(help = "Minimum field overlap percentage")] min_overlap: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<DuplicateTypesReport, String> {
        let root_path = Self::root_path(root);
        let scan_root = target
            .map(PathBuf::from)
            .unwrap_or_else(|| root_path.clone());
        Ok(
            crate::commands::analyze::duplicates::build_duplicate_types_report(
                &scan_root,
                &root_path,
                min_overlap.unwrap_or(70),
            ),
        )
    }

    /// Test/impl line ratio per module
    #[server(group = "test")]
    #[cli(display_with = "display_test_ratio")]
    pub fn test_ratio(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<TestRatioReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::test_ratio::analyze_test_ratio(
            &root_path,
            effective_limit,
        ))
    }

    /// Find untested public functions ranked by risk
    #[server(group = "test")]
    #[cli(display_with = "display_test_gaps")]
    #[allow(clippy::too_many_arguments)]
    pub fn test_gaps(
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
        let filter = Self::build_filter(&root_path, &exclude, &only);
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
        ))
    }

    /// Line budget breakdown by purpose (business logic, tests, docs, config, etc.)
    #[server(group = "test")]
    #[cli(display_with = "display_budget")]
    pub fn budget(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<BudgetReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::budget::analyze_budget(
            &root_path,
            effective_limit,
        ))
    }

    /// Measure information density (compression ratio + token uniqueness) per module
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
        pretty: bool,
        compact: bool,
    ) -> Result<DensityReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let module_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        Ok(crate::commands::analyze::density::analyze_density(
            &root_path,
            module_limit,
            worst.unwrap_or(10),
        ))
    }

    /// Measure what fraction of functions have no structural near-twin per module
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
        pretty: bool,
        compact: bool,
    ) -> Result<UniquenessReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let module_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let filter = Self::build_filter(&root_path, &exclude, &only);
        Ok(crate::commands::analyze::uniqueness::analyze_uniqueness(
            &root_path,
            similarity.unwrap_or(0.80),
            min_lines.unwrap_or(5),
            skeleton,
            include_trait_impls,
            module_limit,
            clusters.unwrap_or(10),
            filter.as_ref(),
        ))
    }

    /// Compute effective (reachable) cyclomatic complexity via call-graph BFS
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
        pretty: bool,
        compact: bool,
    ) -> Result<CallComplexityReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
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

    /// Score each module across test ratio, uniqueness, and density (worst first)
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
        pretty: bool,
        compact: bool,
    ) -> Result<ModuleHealthReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
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

    /// Auto-generated single-page codebase overview
    #[cli(display_with = "display_summary")]
    pub fn summary(
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
        Ok(crate::commands::analyze::summary::analyze_summary(
            &root_path,
            effective_limit,
        ))
    }

    /// Rank modules by import fan-in (requires facts index)
    #[server(group = "modules")]
    #[cli(display_with = "display_imports")]
    pub fn imports(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show only internal (crate-local) modules")] internal: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ImportCentralityReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        crate::commands::analyze::imports::analyze_import_centrality(
            &root_path,
            effective_limit,
            internal,
        )
    }

    /// Per-module dependency depth + ripple risk
    #[server(group = "modules")]
    #[cli(display_with = "display_depth_map")]
    pub fn depth_map(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<DepthMapReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        crate::commands::analyze::depth_map::analyze_depth_map_sync(&root_path, effective_limit)
    }

    /// Graph-theoretic properties of the dependency graph
    #[server(group = "graph")]
    #[cli(display_with = "display_graph")]
    pub fn graph(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Max examples per section (0=no limit)")] limit: Option<usize>,
        #[param(help = "Graph nodes: modules (default) or symbols")] on: Option<GraphTarget>,
        pretty: bool,
        compact: bool,
    ) -> Result<GraphReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let target = on.unwrap_or(GraphTarget::Modules);
        crate::commands::analyze::graph::analyze_graph_sync(&root_path, effective_limit, target)
    }

    /// Per-module public symbol count, public ratio, and constraint score
    #[server(group = "modules")]
    #[cli(display_with = "display_surface")]
    pub fn surface(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<SurfaceReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        crate::commands::analyze::surface::analyze_surface_sync(&root_path, effective_limit)
    }

    /// Per-module import layering compliance — are imports flowing downward?
    #[server(group = "modules")]
    #[cli(display_with = "display_layering")]
    pub fn layering(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<LayeringReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        crate::commands::analyze::layering::analyze_layering_sync(&root_path, effective_limit)
    }

    /// Provenance graph: git blame → session mapping + code relations
    #[server(group = "git")]
    #[cli(display_with = "display_provenance")]
    #[allow(clippy::too_many_arguments)]
    pub fn provenance(
        &self,
        #[param(positional, help = "Target file or directory scope")] target: Option<String>,
        #[param(help = "Include call graph edges (requires facts index)")] calls: bool,
        #[param(help = "Include co-change edges (from git history)")] coupling: bool,
        #[param(help = "Override session directory")] sessions: Option<String>,
        #[param(short = 'l', help = "Maximum number of files (0=no limit)")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<ProvenanceReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(50) {
            0 => usize::MAX,
            n => n,
        };
        let opts = crate::commands::analyze::provenance::ProvenanceOptions {
            target,
            include_calls: calls,
            include_coupling: coupling,
            sessions_path: sessions,
            limit: effective_limit,
        };
        Ok(crate::commands::analyze::provenance::analyze_provenance(
            &root_path, &opts,
        ))
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
        crate::commands::analyze::skeleton_diff::analyze_skeleton_diff(
            &root_path, &base, &exclude, &only,
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

    /// Find repeated AST fragments across the codebase
    #[server(group = "code")]
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
        pretty: bool,
        compact: bool,
    ) -> Result<FragmentsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let scope_val: FragmentScope = scope
            .as_deref()
            .unwrap_or("all")
            .parse()
            .map_err(|e: String| e)?;
        crate::commands::analyze::fragments::analyze_fragments(
            &root_path,
            min_nodes.unwrap_or(10),
            scope_val,
            entry.as_deref(),
            inline_depth.unwrap_or(0),
            similarity.unwrap_or(1.0),
            limit.unwrap_or(30),
            skeleton,
            min_members.unwrap_or(2),
            &exclude,
            &only,
        )
    }
}
