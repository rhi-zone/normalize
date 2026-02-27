//! Analyze sub-service for server-less CLI.

use crate::analyze::complexity::ComplexityReport;
use crate::analyze::function_length::LengthReport;
use crate::commands::analyze::activity::ActivityReport;
use crate::commands::analyze::architecture::ArchitectureReport;
use crate::commands::analyze::call_graph::CallEntry;
use crate::commands::analyze::check_examples::CheckExamplesReport;
use crate::commands::analyze::check_refs::CheckRefsReport;
use crate::commands::analyze::contributors::ContributorsReport;
use crate::commands::analyze::coupling::{CouplingRepoEntry, CouplingReport};
use crate::commands::analyze::docs::DocCoverageReport;
use crate::commands::analyze::duplicates::{
    DuplicateBlocksConfig, DuplicateBlocksReport, DuplicateFunctionsConfig,
    DuplicateFunctionsReport, DuplicateTypesReport, SimilarBlocksConfig, SimilarBlocksReport,
    SimilarFunctionsConfig, SimilarFunctionsReport,
};
use crate::commands::analyze::files::FileLengthReport;
use crate::commands::analyze::hotspots::{HotspotsRepoEntry, HotspotsReport};
use crate::commands::analyze::ownership::{OwnershipRepoEntry, OwnershipReport};
use crate::commands::analyze::query::MatchResult;
use crate::commands::analyze::repo_coupling::RepoCouplingReport;
use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
use crate::commands::analyze::rules_cmd::RulesOutput;
use crate::commands::analyze::stale_docs::StaleDocsReport;
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

fn discover_repos(dir: &str) -> Result<Vec<PathBuf>, String> {
    crate::multi_repo::discover_repos(&PathBuf::from(dir))
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
        r.format_text()
    }

    fn display_stale_docs(&self, r: &StaleDocsReport) -> String {
        r.format_text()
    }

    fn display_check_examples(&self, r: &CheckExamplesReport) -> String {
        r.format_text()
    }

    fn display_ast(&self, v: &serde_json::Value) -> String {
        serde_json::to_string_pretty(v).unwrap_or_default()
    }

    fn display_call_graph(&self, entries: &[CallEntry]) -> String {
        entries
            .iter()
            .map(|e| format!("  {}:{}:{}", e.file, e.line, e.symbol))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn display_query(&self, results: &[MatchResult]) -> String {
        format!("{} matches", results.len())
    }

    fn display_trace(&self, text: &str) -> String {
        text.to_string()
    }

    fn display_rules(&self, out: &RulesOutput) -> String {
        let mut lines = Vec::new();
        if !out.rules.is_empty() {
            lines.push(format!("Rules ({})", out.rules.len()));
            for r in &out.rules {
                lines.push(format!("  {} [{}] - {}", r.id, r.severity, r.message));
            }
        }
        if !out.findings.is_empty() {
            lines.push(format!("\nFindings ({})", out.findings.len()));
            for f in &out.findings {
                lines.push(format!(
                    "  {}:{}: {} [{}]",
                    f.file, f.line, f.message, f.rule_id
                ));
            }
        }
        lines.join("\n")
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
        r.format_text()
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
        r.format_text()
    }

    fn display_file_length(&self, r: &FileLengthReport) -> String {
        r.format_text()
    }

    fn display_hotspots(&self, r: &HotspotsReport) -> String {
        r.format_text()
    }

    fn display_coupling(&self, r: &CouplingReport) -> String {
        r.format_text()
    }

    fn display_ownership(&self, r: &OwnershipReport) -> String {
        r.format_text()
    }

    fn display_contributors(&self, r: &ContributorsReport) -> String {
        r.format_text()
    }

    fn display_activity(&self, r: &ActivityReport) -> String {
        r.format_text()
    }

    fn display_repo_coupling(&self, r: &RepoCouplingReport) -> String {
        r.format_text()
    }

    fn display_dup_functions(&self, r: &DuplicateFunctionsReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_dup_blocks(&self, r: &DuplicateBlocksReport) -> String {
        r.format_text()
    }

    fn display_sim_functions(&self, r: &SimilarFunctionsReport) -> String {
        r.format_text()
    }

    fn display_sim_blocks(&self, r: &SimilarBlocksReport) -> String {
        r.format_text()
    }

    fn display_dup_types(&self, r: &DuplicateTypesReport) -> String {
        r.format_text()
    }

    fn display_test_gaps(&self, r: &crate::analyze::test_gaps::TestGapsReport) -> String {
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
    #[cli(display_with = "display_ast")]
    pub fn ast(
        &self,
        #[param(positional, help = "File to inspect")] file: String,
        #[param(short = 'l', help = "Show node at specific line")] at_line: Option<usize>,
        #[param(help = "Output as S-expression")] sexp: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<serde_json::Value, String> {
        let _root_path = Self::root_path(root);
        let file_path = PathBuf::from(&file);
        let (json, _text) =
            crate::commands::analyze::ast::build_ast_output(&file_path, at_line, sexp)?;
        Ok(json)
    }

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

    /// Run tree-sitter or ast-grep queries against the codebase
    #[cli(display_with = "display_query")]
    pub fn query(
        &self,
        #[param(positional, help = "Query pattern (S-expression or ast-grep)")] pattern: String,
        #[param(short = 'p', help = "Path to search (defaults to root)")] path: Option<String>,
        #[param(help = "Show full source for matches")] show_source: bool,
        #[param(short = 'c', help = "Number of context lines")] context_lines: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<MatchResult>, String> {
        let root_path = Self::root_path(root);
        let search_path = path.map(PathBuf::from);
        crate::commands::analyze::query::run_query_service(
            &pattern,
            search_path.as_deref(),
            show_source,
            context_lines.unwrap_or(5),
            &root_path,
            None,
        )
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

    /// Run syntax rules and report findings
    #[cli(display_with = "display_rules")]
    pub fn rules(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'f', help = "Filter to a specific rule ID")] filter_rule: Option<String>,
        #[param(help = "Only list rules, don't run them")] list_only: bool,
    ) -> Result<RulesOutput, String> {
        let root_path = Self::root_path(root);
        let rules_config = normalize_syntax_rules::RulesConfig(std::collections::HashMap::new());
        let debug = normalize_syntax_rules::DebugFlags::default();
        crate::commands::analyze::rules_cmd::build_rules_output(
            &root_path,
            filter_rule.as_deref(),
            list_only,
            &rules_config,
            &debug,
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
            let idx = crate::index::open_if_enabled(&root_path)
                .await
                .ok_or_else(|| "Indexing disabled or failed.".to_string())?;
            crate::commands::analyze::architecture::analyze_architecture(&idx)
                .await
                .map_err(|e| format!("Architecture analysis failed: {}", e))
        })
    }

    /// Run health analysis (file counts, complexity stats, large file warnings)
    #[cli(display_with = "display_report")]
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
        #[param(short = 'l', help = "Number of worst-covered files to show")] limit: Option<usize>,
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
        #[param(short = 'l', help = "Number of files to show")] limit: Option<usize>,
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

    /// Show git history hotspots (frequently changed files)
    #[cli(display_with = "display_hotspots")]
    pub fn hotspots(
        &self,
        #[param(help = "Weight recent changes higher")] recency: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Run across all git repos under DIR (1 level deep)")] repos: Option<String>,
    ) -> Result<HotspotsReport, String> {
        let root_path = Self::root_path(root);
        if let Some(repos_dir) = repos {
            let repo_paths = discover_repos(&repos_dir)?;
            let entries: Vec<HotspotsRepoEntry> = repo_paths
                .into_iter()
                .map(|repo_path| {
                    let name = repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let config = crate::config::NormalizeConfig::load(&repo_path);
                    let mut excludes = config.analyze.hotspots_exclude.clone();
                    excludes.extend(crate::commands::analyze::load_allow_file(
                        &repo_path,
                        "hotspots-allow",
                    ));
                    match crate::commands::analyze::hotspots::analyze_hotspots(
                        &repo_path, &excludes, recency,
                    ) {
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
        let config = crate::config::NormalizeConfig::load(&root_path);
        let mut excludes = config.analyze.hotspots_exclude.clone();
        excludes.extend(crate::commands::analyze::load_allow_file(
            &root_path,
            "hotspots-allow",
        ));
        crate::commands::analyze::hotspots::analyze_hotspots(&root_path, &excludes, recency)
    }

    /// Find files that frequently change together (temporal coupling)
    #[cli(display_with = "display_coupling")]
    pub fn coupling(
        &self,
        #[param(help = "Minimum number of shared commits to report a pair")] min_commits: Option<
            usize,
        >,
        #[param(short = 'l', help = "Maximum number of pairs to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Run across all git repos under DIR (1 level deep)")] repos: Option<String>,
    ) -> Result<CouplingReport, String> {
        let root_path = Self::root_path(root);
        let min = min_commits.unwrap_or(3);
        let lim = limit.unwrap_or(20);
        if let Some(repos_dir) = repos {
            let repo_paths = discover_repos(&repos_dir)?;
            let entries: Vec<CouplingRepoEntry> = repo_paths
                .into_iter()
                .map(|repo_path| {
                    let name = repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    match crate::commands::analyze::coupling::analyze_coupling(
                        &repo_path, min, lim, &exclude,
                    ) {
                        Ok(r) => CouplingRepoEntry {
                            name,
                            error: None,
                            pairs: r.pairs,
                        },
                        Err(e) => CouplingRepoEntry {
                            name,
                            error: Some(e),
                            pairs: vec![],
                        },
                    }
                })
                .collect();
            return Ok(CouplingReport {
                pairs: vec![],
                repos: Some(entries),
            });
        }
        crate::commands::analyze::coupling::analyze_coupling(&root_path, min, lim, &exclude)
    }

    /// Show per-file ownership concentration from git blame
    #[cli(display_with = "display_ownership")]
    pub fn ownership(
        &self,
        #[param(short = 'l', help = "Maximum number of files to show")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Run across all git repos under DIR (1 level deep)")] repos: Option<String>,
    ) -> Result<OwnershipReport, String> {
        let root_path = Self::root_path(root);
        let lim = limit.unwrap_or(20);
        if let Some(repos_dir) = repos {
            let repo_paths = discover_repos(&repos_dir)?;
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
        #[param(help = "Directory containing git repos (1 level deep)")] repos_dir: String,
    ) -> Result<ContributorsReport, String> {
        let repos = discover_repos(&repos_dir)?;
        crate::commands::analyze::contributors::analyze_contributors(&repos)
    }

    /// Analyze cross-repo activity over time
    #[cli(display_with = "display_activity")]
    pub fn activity(
        &self,
        #[param(help = "Directory containing git repos (1 level deep)")] repos_dir: String,
        #[param(help = "Window granularity: month (default) or week")] window: Option<String>,
        #[param(help = "Number of windows to show")] windows: Option<usize>,
    ) -> Result<ActivityReport, String> {
        let repos = discover_repos(&repos_dir)?;
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
        #[param(help = "Directory containing git repos (1 level deep)")] repos_dir: String,
        #[param(help = "Window size in hours for temporal grouping")] window: Option<usize>,
        #[param(help = "Minimum shared windows to report a temporal pair")] min_windows: Option<
            usize,
        >,
    ) -> Result<RepoCouplingReport, String> {
        let repos = discover_repos(&repos_dir)?;
        crate::commands::analyze::repo_coupling::analyze_repo_coupling(
            &repos,
            window.unwrap_or(24),
            min_windows.unwrap_or(3),
        )
    }

    /// Detect duplicate functions (code clones)
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
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<DuplicateFunctionsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        let dummy_format = crate::output::OutputFormat::default();
        Ok(
            crate::commands::analyze::duplicates::build_duplicate_functions_report(
                DuplicateFunctionsConfig {
                    root: &root_path,
                    elide_identifiers, // true by default in existing CLI; server-less bool flags default false
                    elide_literals,
                    show_source,
                    min_lines: min_lines.unwrap_or(1),
                    include_trait_impls,
                    format: &dummy_format,
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
        let dummy_format = crate::output::OutputFormat::default();
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
                    format: &dummy_format,
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
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SimilarFunctionsReport, String> {
        let root_path = Self::root_path(root);
        self.resolve_format(pretty, compact, &root_path);
        let filter = Self::build_filter(&root_path, &exclude, &only);
        let dummy_format = crate::output::OutputFormat::default();
        Ok(
            crate::commands::analyze::duplicates::build_similar_functions_report(
                SimilarFunctionsConfig {
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
                    format: &dummy_format,
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
        let dummy_format = crate::output::OutputFormat::default();
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
                    format: &dummy_format,
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

    /// Find public functions with no direct test caller
    #[cli(display_with = "display_test_gaps")]
    #[allow(clippy::too_many_arguments)]
    pub fn test_gaps(
        &self,
        #[param(positional, help = "Target file or directory")] target: Option<String>,
        #[param(help = "Show all functions including tested")] all: bool,
        #[param(help = "Only show functions above this risk threshold")] min_risk: Option<f64>,
        #[param(short = 'l', help = "Maximum number of functions to show (0=no limit)")]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<crate::analyze::test_gaps::TestGapsReport, String> {
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
}
