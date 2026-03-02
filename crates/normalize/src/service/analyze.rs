//! Analyze sub-service for server-less CLI.

use crate::analyze::complexity::ComplexityReport;
use crate::analyze::function_length::LengthReport;
use crate::commands::analyze::activity::ActivityReport;
use crate::commands::analyze::architecture::ArchitectureReport;
use crate::commands::analyze::call_complexity::CallComplexityReport;
use crate::commands::analyze::call_graph::CallEntry;
use crate::commands::analyze::ceremony::CeremonyReport;
use crate::commands::analyze::check_examples::CheckExamplesReport;
use crate::commands::analyze::check_refs::CheckRefsReport;
use crate::commands::analyze::clusters::ClustersReport;
use crate::commands::analyze::contributors::ContributorsReport;
use crate::commands::analyze::coupling_views::CouplingOutput;
use crate::commands::analyze::coverage::CoverageOutput;
use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
use crate::commands::analyze::density::DensityReport;
use crate::commands::analyze::depth_map::DepthMapReport;
use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::duplicates::{
    DuplicateBlocksConfig, DuplicateBlocksReport, DuplicateFunctionsConfig,
    DuplicateFunctionsReport, DuplicateTypesReport, SimilarBlocksConfig, SimilarBlocksReport,
    SimilarFunctionsConfig, SimilarFunctionsReport,
};
use crate::commands::analyze::files::FileLengthReport;
use crate::commands::analyze::impact::ImpactReport;
use crate::commands::analyze::imports::ImportCentralityReport;
use crate::commands::analyze::layering::LayeringReport;
use crate::commands::analyze::module_health::ModuleHealthReport;
use crate::commands::analyze::ownership::{OwnershipRepoEntry, OwnershipReport};
use crate::commands::analyze::patterns::PatternsReport;
use crate::commands::analyze::provenance::ProvenanceReport;
use crate::commands::analyze::repo_coupling::RepoCouplingReport;
use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
use crate::commands::analyze::size::SizeReport;
use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
use crate::commands::analyze::stale_docs::StaleDocsReport;
use crate::commands::analyze::summary::SummaryReport;
use crate::commands::analyze::surface::SurfaceReport;
use crate::commands::analyze::trend::TrendReport;
use crate::commands::analyze::uniqueness::UniquenessReport;
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
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    fn display_check_refs(&self, r: &CheckRefsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_stale_docs(&self, r: &StaleDocsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_check_examples(&self, r: &CheckExamplesReport) -> String {
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

    fn display_clusters(&self, r: &ClustersReport) -> String {
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

    fn display_dup_functions(&self, r: &DuplicateFunctionsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_dup_blocks(&self, r: &DuplicateBlocksReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_sim_functions(&self, r: &SimilarFunctionsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_sim_blocks(&self, r: &SimilarBlocksReport) -> String {
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

    fn display_patterns(&self, r: &PatternsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_coverage(&self, r: &CoverageOutput) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_coupling_output(&self, r: &CouplingOutput) -> String {
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
impl AnalyzeService {
    /// Check for broken documentation references
    #[cli(display_with = "display_check_refs")]
    pub fn check_refs(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<CheckRefsReport, String> {
        let root_path = Self::root_path(root);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(crate::commands::analyze::check_refs::build_check_refs_report(&root_path))
    }

    /// Check for stale documentation
    #[cli(display_with = "display_stale_docs")]
    pub fn stale_docs(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<StaleDocsReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::stale_docs::build_stale_docs_report(&root_path))
    }

    /// Check for missing example references in documentation
    #[cli(display_with = "display_check_examples")]
    pub fn check_examples(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<CheckExamplesReport, String> {
        let root_path = Self::root_path(root);
        Ok(crate::commands::analyze::check_examples::build_check_examples_report(&root_path))
    }

    /// Show AST structure for a file
    /// Show callers and/or callees of a symbol
    #[cli(display_with = "display_call_graph")]
    pub fn call_graph(
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
        let show_callees = callees;
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &target,
            show_callers,
            show_callees,
            case_insensitive,
        ))
    }

    /// Trace value provenance for a symbol
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
    #[cli(display_with = "display_architecture")]
    pub fn architecture(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ArchitectureReport, String> {
        let root_path = Self::root_path(root);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(async {
            let idx = crate::index::ensure_ready(&root_path).await?;
            crate::commands::analyze::architecture::analyze_architecture(&idx)
                .await
                .map_err(|e| format!("Architecture analysis failed: {}", e))
        })
    }

    /// What-if impact analysis: reverse-dependency closure + blast radius
    #[cli(display_with = "display_impact")]
    pub fn impact(
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
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(async {
            let idx = crate::index::ensure_ready(&root_path).await?;
            crate::commands::analyze::impact::analyze_impact(&idx, &target)
                .await
                .map_err(|e| format!("Impact analysis failed: {}", e))
        })
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

    /// Group similar functions into structural clusters (connected components of similar-functions pairs)
    #[cli(display_with = "display_clusters")]
    #[allow(clippy::too_many_arguments)]
    pub fn clusters(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Minimum lines for a function to be considered")] min_lines: Option<usize>,
        #[param(help = "Minimum similarity threshold (0.0–1.0)")] similarity: Option<f64>,
        #[param(short = 'l', help = "Maximum number of clusters to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Match on control-flow structure only")] skeleton: bool,
        #[param(help = "Include same-name clusters (likely interface implementations)")]
        include_trait_impls: bool,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
    ) -> Result<ClustersReport, String> {
        let root_path = Self::root_path(root);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        Ok(crate::commands::analyze::clusters::build_clusters_report(
            &root_path,
            min_lines.unwrap_or(10),
            similarity.unwrap_or(0.85),
            skeleton,
            include_trait_impls,
            limit.unwrap_or(20),
            filter.as_ref(),
        ))
    }

    /// Temporal churn analysis: coupling pairs, change-clusters, or hotspots
    ///
    /// Default: coupling pairs (files that change together).
    /// Use --cluster for change-clusters (connected components).
    /// Use --hotspots for churn × complexity hotspots.
    #[cli(display_with = "display_coupling_output")]
    #[allow(clippy::too_many_arguments)]
    pub fn churn(
        &self,
        #[param(help = "Show change-clusters (connected components)")] cluster: bool,
        #[param(help = "Show churn × complexity hotspots")] hotspots: bool,
        #[param(help = "Minimum shared commits for coupling/cluster edges")] min_commits: Option<
            usize,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Weight recent changes higher (hotspots view)")] recency: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CouplingOutput, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        if hotspots {
            let config = crate::config::NormalizeConfig::load(&root_path);
            let mut excludes = config.analyze.hotspots_exclude.clone();
            excludes.extend(crate::commands::analyze::load_allow_file(
                &root_path,
                "hotspots-allow",
            ));
            Ok(CouplingOutput::Hotspots(
                crate::commands::analyze::hotspots::analyze_hotspots(
                    &root_path, &excludes, recency,
                )?,
            ))
        } else if cluster {
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
            Ok(CouplingOutput::Clusters(
                crate::commands::analyze::coupling_clusters::analyze_coupling_clusters(
                    &root_path,
                    effective_min,
                    limit.unwrap_or(20),
                    &exclude,
                    &only,
                )?,
            ))
        } else {
            let min = min_commits.unwrap_or(3);
            let lim = limit.unwrap_or(20);
            Ok(CouplingOutput::Pairs(
                crate::commands::analyze::coupling::analyze_coupling(
                    &root_path, min, lim, &exclude,
                )?,
            ))
        }
    }

    /// Show per-file ownership concentration from git blame
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
    #[cli(display_with = "display_activity")]
    pub fn activity(
        &self,
        #[param(help = "Directory containing git repos")] repos_dir: String,
        #[param(help = "Window granularity: month (default) or week")] window: Option<String>,
        #[param(help = "Number of windows to show")] windows: Option<usize>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
    ) -> Result<ActivityReport, String> {
        let repos = discover_repos(&repos_dir, repos_depth.unwrap_or(1))?;
        crate::commands::analyze::activity::analyze_activity(
            &repos,
            window.as_deref().unwrap_or("month"),
            windows.unwrap_or(12),
        )
    }

    /// Analyze cross-repo coupling
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

    /// Detect duplicate functions (code clones)
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

    #[cli(display_with = "display_dup_functions")]
    #[allow(clippy::too_many_arguments)]
    pub fn duplicate_functions(
        &self,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing")] elide_literals: bool,
        #[param(help = "Show source code for detected duplicates")] show_source: bool,
        #[param(help = "Minimum lines for a function to be considered")] min_lines: Option<usize>,
        #[param(help = "Include groups where all functions share the same name")]
        include_trait_impls: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Scan across all git repos under DIR")] repos: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<DuplicateFunctionsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let roots: Vec<PathBuf> = if let Some(repos_dir) = repos {
            discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
        } else {
            vec![root_path.clone()]
        };
        let filter = Self::build_filter(&root_path, &exclude, &only);
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

    /// Detect duplicate code blocks
    #[cli(display_with = "display_dup_blocks")]
    #[allow(clippy::too_many_arguments)]
    pub fn duplicate_blocks(
        &self,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing")] elide_literals: bool,
        #[param(help = "Show source code for detected duplicates")] show_source: bool,
        #[param(help = "Minimum lines for a block to be considered")] min_lines: Option<usize>,
        #[param(help = "Skip function/method nodes")] skip_functions: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<DuplicateBlocksReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        Ok(
            crate::commands::analyze::duplicates::build_duplicate_blocks_report(
                DuplicateBlocksConfig {
                    root: &root_path,
                    min_lines: min_lines.unwrap_or(5),
                    elide_identifiers, // true by default in existing CLI; server-less bool flags default false
                    elide_literals,
                    skip_functions,
                    show_source,
                    allow: None,
                    reason: None,
                    filter: filter.as_ref(),
                },
            ),
        )
    }

    /// Detect similar functions via MinHash LSH
    #[cli(display_with = "display_sim_functions")]
    #[allow(clippy::too_many_arguments)]
    pub fn similar_functions(
        &self,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing")] elide_literals: bool,
        #[param(help = "Show source code for matches")] show_source: bool,
        #[param(help = "Minimum lines for a function to be considered")] min_lines: Option<usize>,
        #[param(help = "Minimum similarity threshold (0.0-1.0)")] similarity: Option<f64>,
        #[param(help = "Match on control-flow structure")] skeleton: bool,
        #[param(help = "Include pairs where both functions share the same name")]
        include_trait_impls: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Scan across all git repos under DIR")] repos: Option<String>,
        #[param(help = "Max depth to search for repos (default: 1)")] repos_depth: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SimilarFunctionsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let roots: Vec<PathBuf> = if let Some(repos_dir) = repos {
            discover_repos(&repos_dir, repos_depth.unwrap_or(1))?
        } else {
            vec![root_path.clone()]
        };
        let filter = Self::build_filter(&root_path, &exclude, &only);
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

    /// Detect similar code blocks via MinHash LSH
    #[cli(display_with = "display_sim_blocks")]
    #[allow(clippy::too_many_arguments)]
    pub fn similar_blocks(
        &self,
        #[param(help = "Elide identifier names when comparing")] elide_identifiers: bool,
        #[param(help = "Elide literal values when comparing")] elide_literals: bool,
        #[param(help = "Show source code for matches")] show_source: bool,
        #[param(help = "Minimum lines for a block to be considered")] min_lines: Option<usize>,
        #[param(help = "Minimum similarity threshold (0.0-1.0)")] similarity: Option<f64>,
        #[param(help = "Match on control-flow structure")] skeleton: bool,
        #[param(help = "Include pairs where both blocks are inside same-name functions")]
        include_trait_impls: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SimilarBlocksReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        Ok(
            crate::commands::analyze::duplicates::build_similar_blocks_report(
                SimilarBlocksConfig {
                    root: &root_path,
                    min_lines: min_lines.unwrap_or(10),
                    similarity: similarity.unwrap_or(0.85),
                    elide_identifiers, // true by default in existing CLI; server-less bool flags default false
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

    /// Detect duplicate type definitions
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

    /// Test coverage analysis: ratio, gaps, or budget view
    ///
    /// Default: test-ratio (test/impl line ratio per module).
    /// Use --gaps to find untested public functions.
    /// Use --budget for line budget breakdown by purpose.
    #[cli(display_with = "display_coverage")]
    #[allow(clippy::too_many_arguments)]
    pub fn coverage(
        &self,
        #[param(positional, help = "Target file or directory (for gaps view)")] target: Option<
            String,
        >,
        #[param(help = "Show untested public functions")] gaps: bool,
        #[param(help = "Show line budget breakdown by purpose")] budget: bool,
        #[param(help = "Show all functions including tested (gaps view)")] all: bool,
        #[param(help = "Only show functions above this risk threshold (gaps view)")]
        min_risk: Option<f64>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of entries to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CoverageOutput, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        if gaps {
            let filter = Self::build_filter(&root_path, &exclude, &only);
            let allowlist =
                crate::commands::analyze::load_allow_file(&root_path, "test-gaps-allow");
            let effective_limit = match limit.unwrap_or(20) {
                0 => usize::MAX,
                n => n,
            };
            Ok(CoverageOutput::Gaps(
                crate::commands::analyze::test_gaps::analyze_test_gaps(
                    &root_path,
                    target.as_deref(),
                    all,
                    min_risk,
                    effective_limit,
                    filter.as_ref(),
                    &allowlist,
                ),
            ))
        } else if budget {
            let effective_limit = match limit.unwrap_or(30) {
                0 => usize::MAX,
                n => n,
            };
            Ok(CoverageOutput::Budget(
                crate::commands::analyze::budget::analyze_budget(&root_path, effective_limit),
            ))
        } else {
            let effective_limit = match limit.unwrap_or(30) {
                0 => usize::MAX,
                n => n,
            };
            Ok(CoverageOutput::Ratio(
                crate::commands::analyze::test_ratio::analyze_test_ratio(
                    &root_path,
                    effective_limit,
                ),
            ))
        }
    }

    /// Measure information density (compression ratio + token uniqueness) per module
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

    /// Per-module public symbol count, public ratio, and constraint score
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

    /// Show callers of a symbol
    #[cli(display_with = "display_call_graph")]
    pub fn callers(
        &self,
        #[param(positional, help = "Symbol to find callers for")] symbol: String,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<CallEntry>, String> {
        let root_path = Self::root_path(root);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &symbol,
            true,
            false,
            case_insensitive,
        ))
    }

    /// Show what functions a symbol calls
    #[cli(display_with = "display_call_graph")]
    pub fn callees(
        &self,
        #[param(positional, help = "Symbol to find callees for")] symbol: String,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<CallEntry>, String> {
        let root_path = Self::root_path(root);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &symbol,
            false,
            true,
            case_insensitive,
        ))
    }

    /// Show structural changes between a base ref and HEAD
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

    /// Auto-detect recurring structural code patterns
    #[cli(display_with = "display_patterns")]
    #[allow(clippy::too_many_arguments)]
    pub fn patterns(
        &self,
        #[param(short = 's', help = "Minimum structural similarity (default: 0.7)")]
        similarity: Option<f64>,
        #[param(short = 'm', help = "Minimum cluster size to report (default: 3)")]
        min_members: Option<usize>,
        #[param(
            short = 'l',
            help = "Maximum number of patterns to show (0=no limit, default: 20)"
        )]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<PatternsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        crate::commands::analyze::patterns::analyze_patterns(
            &root_path,
            similarity.unwrap_or(0.7),
            min_members.unwrap_or(3),
            limit.unwrap_or(20),
            &exclude,
            &only,
        )
    }
}
