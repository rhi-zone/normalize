//! Analyze command - run analysis on target.

mod duplicates;
mod trace;

use crate::analysis_report;
use crate::analyze::complexity::ComplexityReport;
use crate::commands::filter::detect_project_languages;
use crate::config::MossConfig;
use crate::daemon;
use crate::filter::Filter;
use crate::index;
use crate::merge::Merge;
use crate::overview;
use crate::path_resolve;
use clap::Args;
use moss_tools::registry_with_custom;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Analyze command configuration.
#[derive(Debug, Clone, Deserialize, Default, Merge)]
#[serde(default)]
pub struct AnalyzeConfig {
    /// Default complexity threshold for filtering
    pub threshold: Option<usize>,
    /// Use compact output by default (for --overview)
    pub compact: Option<bool>,
    /// Run health analysis by default
    pub health: Option<bool>,
    /// Run complexity analysis by default
    pub complexity: Option<bool>,
    /// Run security analysis by default
    pub security: Option<bool>,
    /// Run duplicate function detection by default
    pub duplicate_functions: Option<bool>,
    /// Weights for final grade calculation
    pub weights: Option<AnalyzeWeights>,
}

/// Weights for each analysis pass (higher = more impact on grade).
#[derive(Debug, Clone, Deserialize, Default, Merge)]
#[serde(default)]
pub struct AnalyzeWeights {
    pub health: Option<f64>,
    pub complexity: Option<f64>,
    pub security: Option<f64>,
    pub duplicate_functions: Option<f64>,
}

impl AnalyzeWeights {
    pub fn health(&self) -> f64 {
        self.health.unwrap_or(1.0)
    }
    pub fn complexity(&self) -> f64 {
        self.complexity.unwrap_or(0.5)
    }
    pub fn security(&self) -> f64 {
        self.security.unwrap_or(2.0)
    }
    pub fn duplicate_functions(&self) -> f64 {
        self.duplicate_functions.unwrap_or(0.3)
    }
}

impl AnalyzeConfig {
    pub fn threshold(&self) -> Option<usize> {
        self.threshold
    }

    pub fn compact(&self) -> bool {
        self.compact.unwrap_or(false)
    }

    pub fn health(&self) -> bool {
        self.health.unwrap_or(true)
    }

    pub fn complexity(&self) -> bool {
        self.complexity.unwrap_or(true)
    }

    pub fn security(&self) -> bool {
        self.security.unwrap_or(true)
    }

    pub fn duplicate_functions(&self) -> bool {
        self.duplicate_functions.unwrap_or(false)
    }

    pub fn weights(&self) -> AnalyzeWeights {
        self.weights.clone().unwrap_or_default()
    }
}

/// Analyze command arguments.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Target to analyze (path, file, or directory)
    pub target: Option<String>,

    /// Root directory (defaults to current directory)
    #[arg(short, long)]
    pub root: Option<PathBuf>,

    /// Run all analysis passes including duplicate function detection
    #[arg(long)]
    pub all: bool,

    /// Run health analysis
    #[arg(long)]
    pub health: bool,

    /// Run complexity analysis
    #[arg(long)]
    pub complexity: bool,

    /// Run function length analysis
    #[arg(long)]
    pub length: bool,

    /// Run security analysis
    #[arg(long)]
    pub security: bool,

    /// Show comprehensive project overview
    #[arg(long)]
    pub overview: bool,

    /// Compact one-line output (for --overview)
    #[arg(short, long)]
    pub compact: bool,

    /// Complexity threshold - only show functions above this
    #[arg(short, long)]
    pub threshold: Option<usize>,

    /// Filter by symbol kind: function, method
    #[arg(long)]
    pub kind: Option<String>,

    /// Show what functions the target calls
    #[arg(long)]
    pub callees: bool,

    /// Show what functions call the target
    #[arg(long)]
    pub callers: bool,

    /// Run linters
    #[arg(long)]
    pub lint: bool,

    /// Show git history hotspots
    #[arg(long)]
    pub hotspots: bool,

    /// Check documentation references
    #[arg(long)]
    pub check_refs: bool,

    /// Find docs with stale code references
    #[arg(long)]
    pub stale_docs: bool,

    /// Check example references
    #[arg(long)]
    pub check_examples: bool,

    /// Detect duplicate functions (code clones)
    #[arg(long)]
    pub duplicate_functions: bool,

    /// Detect duplicate type definitions (structs with similar fields)
    #[arg(long)]
    pub duplicate_types: bool,

    /// Minimum field overlap percentage for duplicate type detection (default: 70)
    #[arg(long, default_value = "70")]
    pub min_overlap: usize,

    /// Allow a duplicate type pair (add to .moss/duplicate-types-allow)
    #[arg(long, num_args = 2, value_names = ["TYPE1", "TYPE2"])]
    pub allow_type: Option<Vec<String>>,

    /// Elide identifier names when detecting duplicate functions (default: true)
    #[arg(long, default_value = "true")]
    pub elide_identifiers: bool,

    /// Elide literal values when detecting duplicate functions (default: false)
    #[arg(long)]
    pub elide_literals: bool,

    /// Show source code for detected duplicate functions
    #[arg(long)]
    pub show_source: bool,

    /// Minimum lines for a function to be considered for duplicate detection
    #[arg(long, default_value = "1")]
    pub min_lines: usize,

    /// Allow a duplicate function group by location (add to .moss/duplicate-functions-allow). Format: path:symbol
    #[arg(long, value_name = "LOCATION")]
    pub allow_function: Option<String>,

    /// Reason for allowing (required for new groups or type pairs)
    #[arg(long, value_name = "REASON")]
    pub reason: Option<String>,

    /// Exclude paths matching pattern or @alias
    #[arg(long, value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Include only paths matching pattern or @alias
    #[arg(long, value_name = "PATTERN")]
    pub only: Vec<String>,

    /// Trace value provenance for a symbol
    #[arg(long, value_name = "SYMBOL")]
    pub trace: Option<String>,

    /// Maximum trace depth (default: 10)
    #[arg(long, default_value = "10")]
    pub max_depth: usize,
}

/// Run analyze command with args.
pub fn run(args: AnalyzeArgs, format: crate::output::OutputFormat) -> i32 {
    let effective_root = args
        .root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let config = MossConfig::load(&effective_root);

    // Handle --allow-function mode
    if let Some(ref location) = args.allow_function {
        return duplicates::cmd_allow_duplicate_function(
            &effective_root,
            location,
            args.reason.as_deref(),
            args.elide_identifiers,
            args.elide_literals,
            args.min_lines,
        );
    }

    // Determine which passes to run:
    // --all: run everything
    // Specific flags: run only those
    // No flags: use config defaults
    let (health, complexity, length, security, duplicate_functions) = if args.all {
        (true, true, true, true, true)
    } else {
        let any_pass_flag = args.health
            || args.complexity
            || args.length
            || args.security
            || args.duplicate_functions;
        if any_pass_flag {
            (
                args.health,
                args.complexity,
                args.length,
                args.security,
                args.duplicate_functions,
            )
        } else {
            (
                config.analyze.health(),
                config.analyze.complexity(),
                false, // length off by default
                config.analyze.security(),
                config.analyze.duplicate_functions(),
            )
        }
    };

    let weights = config.analyze.weights();

    // Handle --allow-type mode
    if let Some(ref types) = args.allow_type {
        if types.len() == 2 {
            return duplicates::cmd_allow_duplicate_type(
                &effective_root,
                &types[0],
                &types[1],
                args.reason.as_deref(),
            );
        }
    }

    // Handle --duplicate-types as standalone pass
    if args.duplicate_types {
        let scan_root = args
            .target
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| effective_root.clone());
        return duplicates::cmd_duplicate_types(
            &scan_root,
            &effective_root,
            args.min_overlap,
            format.is_json(),
        );
    }

    // Handle --trace mode
    if let Some(ref symbol) = args.trace {
        return trace::cmd_trace(
            symbol,
            args.target.as_deref(),
            &effective_root,
            args.max_depth,
            format.is_json(),
            format.is_pretty(),
        );
    }

    cmd_analyze(
        args.target.as_deref(),
        args.root.as_deref(),
        health,
        complexity,
        length,
        security,
        args.overview,
        args.compact || config.analyze.compact(),
        args.threshold.or(config.analyze.threshold()),
        args.kind.as_deref(),
        args.callees,
        args.callers,
        args.lint,
        args.hotspots,
        args.check_refs,
        args.stale_docs,
        args.check_examples,
        duplicate_functions,
        args.elide_identifiers,
        args.elide_literals,
        args.show_source,
        args.min_lines,
        &weights,
        format.is_json(),
        format.is_pretty(),
        &args.exclude,
        &args.only,
    )
}

/// Run analysis on a target (file or directory)
#[allow(clippy::too_many_arguments)]
pub fn cmd_analyze(
    target: Option<&str>,
    root: Option<&Path>,
    health: bool,
    complexity: bool,
    length: bool,
    security: bool,
    show_overview: bool,
    compact: bool,
    threshold: Option<usize>,
    kind_filter: Option<&str>,
    callees: bool,
    callers: bool,
    lint: bool,
    hotspots: bool,
    check_refs: bool,
    stale_docs: bool,
    check_examples: bool,
    duplicate_functions: bool,
    elide_identifiers: bool,
    elide_literals: bool,
    show_source: bool,
    min_lines: usize,
    weights: &AnalyzeWeights,
    json: bool,
    pretty: bool,
    exclude: &[String],
    only: &[String],
) -> i32 {
    // --overview runs the overview report
    if show_overview {
        return cmd_overview(root, compact, json);
    }

    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Ensure daemon is running if configured
    daemon::maybe_start_daemon(&root);

    // Build filter for --exclude and --only
    let filter = if !exclude.is_empty() || !only.is_empty() {
        let config = MossConfig::load(&root);
        let languages = detect_project_languages(&root);
        let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();

        match Filter::new(exclude, only, &config.filter, &lang_refs) {
            Ok(f) => {
                for warning in f.warnings() {
                    eprintln!("warning: {}", warning);
                }
                Some(f)
            }
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        }
    } else {
        None
    };

    // --callees or --callers: show call graph info
    if callees || callers {
        let target = match target {
            Some(t) => t,
            None => {
                eprintln!("--callees and --callers require a target symbol");
                return 1;
            }
        };
        return cmd_call_graph(&root, target, callers, callees, json);
    }

    // --lint runs linter analysis
    if lint {
        return cmd_lint_analyze(&root, target, json);
    }

    // --hotspots runs git history hotspot analysis
    if hotspots {
        return cmd_hotspots(&root, json);
    }

    // --check-refs validates documentation references
    if check_refs {
        return cmd_check_refs(&root, json);
    }

    // --stale-docs finds docs where covered code has changed
    if stale_docs {
        return cmd_stale_docs(&root, json);
    }

    // --check-examples validates example references
    if check_examples {
        return cmd_check_examples(&root, json);
    }

    let mut exit_code = 0;
    let mut scores: Vec<(f64, f64)> = Vec::new(); // (score, weight)

    // Run main analysis if any of health/complexity/length/security enabled
    if health || complexity || length || security {
        let report = analysis_report::analyze(
            target,
            &root,
            health,
            complexity,
            length,
            security,
            threshold,
            kind_filter,
            filter.as_ref(),
        );

        // Collect scores from report
        if let Some(ref complexity_report) = report.complexity {
            let score = score_complexity(complexity_report);
            scores.push((score, weights.complexity()));
        }
        if let Some(ref security_report) = report.security {
            let score = score_security(security_report);
            scores.push((score, weights.security()));
        }

        if json {
            println!("{}", report.to_json());
        } else if pretty {
            println!("{}", report.format_pretty());
        } else {
            println!("{}", report.format());
        }
    }

    // Run duplicate function detection if enabled
    if duplicate_functions {
        let (result, count) = duplicates::cmd_duplicate_functions_with_count(
            &root,
            elide_identifiers,
            elide_literals,
            show_source,
            min_lines,
            json,
        );
        scores.push((
            score_duplicate_functions(count),
            weights.duplicate_functions(),
        ));
        if result != 0 {
            exit_code = result;
        }
    }

    // Output final grade if we ran any passes
    if !scores.is_empty() && !json {
        let (grade, percentage) = calculate_grade(&scores);
        println!();
        println!("# Overall Grade: {} ({:.0}%)", grade, percentage);
    }

    exit_code
}

/// Score complexity: 100 if no high-risk functions, decreases with complex code
fn score_complexity(report: &ComplexityReport) -> f64 {
    let high_risk = report.high_risk_count();
    let total = report.functions.len();
    if total == 0 {
        return 100.0;
    }
    let ratio = high_risk as f64 / total as f64;
    (100.0 * (1.0 - ratio)).max(0.0)
}

/// Score security: 100 if no findings, penalized by severity
fn score_security(report: &analysis_report::SecurityReport) -> f64 {
    let counts = report.count_by_severity();
    let penalty =
        counts["critical"] * 40 + counts["high"] * 20 + counts["medium"] * 10 + counts["low"] * 5;
    (100.0 - penalty as f64).max(0.0)
}

/// Score duplicate functions: 100 if none, -5 per group
fn score_duplicate_functions(groups: usize) -> f64 {
    (100.0 - (groups * 5) as f64).max(0.0)
}

/// Calculate weighted average grade from scores
fn calculate_grade(scores: &[(f64, f64)]) -> (&'static str, f64) {
    let total_weight: f64 = scores.iter().map(|(_, w)| w).sum();
    if total_weight == 0.0 {
        return ("N/A", 0.0);
    }
    let weighted_sum: f64 = scores.iter().map(|(s, w)| s * w).sum();
    let percentage = weighted_sum / total_weight;

    let grade = match percentage as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };
    (grade, percentage)
}

/// Run linter analysis on the codebase
fn cmd_lint_analyze(root: &Path, target: Option<&str>, json: bool) -> i32 {
    let registry = registry_with_custom(root);
    let detected = registry.detect(root);

    if detected.is_empty() {
        if json {
            println!("{{\"tools\": [], \"summary\": {{\"errors\": 0, \"warnings\": 0}}}}");
        } else {
            eprintln!("No relevant linting tools found for this project.");
        }
        return 0;
    }

    let paths: Vec<&Path> = target.map(|t| vec![Path::new(t)]).unwrap_or_default();
    let mut all_results = Vec::new();
    let mut tools_run = Vec::new();

    for (tool, _reason) in &detected {
        let info = tool.info();

        if !tool.is_available() {
            continue;
        }

        if !json {
            eprintln!("{}: checking...", info.name);
        }

        match tool.run(&paths.iter().copied().collect::<Vec<_>>(), root) {
            Ok(result) => {
                tools_run.push(info.name);
                all_results.push(result);
            }
            Err(e) => {
                if !json {
                    eprintln!("{}: {}", info.name, e);
                }
            }
        }
    }

    let total_errors: usize = all_results.iter().map(|r| r.error_count()).sum();
    let total_warnings: usize = all_results.iter().map(|r| r.warning_count()).sum();

    if json {
        let diagnostics = moss_tools::ToolRegistry::collect_diagnostics(&all_results);
        let output = serde_json::json!({
            "tools": tools_run,
            "summary": {
                "errors": total_errors,
                "warnings": total_warnings,
            },
            "results": all_results.iter().map(|r| {
                serde_json::json!({
                    "tool": r.tool,
                    "success": r.success,
                    "errors": r.error_count(),
                    "warnings": r.warning_count(),
                })
            }).collect::<Vec<_>>(),
            "diagnostics": diagnostics,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        // Print diagnostics
        for result in &all_results {
            for diag in &result.diagnostics {
                let severity = match diag.severity {
                    moss_tools::DiagnosticSeverity::Error => "error",
                    moss_tools::DiagnosticSeverity::Warning => "warning",
                    moss_tools::DiagnosticSeverity::Info => "info",
                    moss_tools::DiagnosticSeverity::Hint => "hint",
                };

                println!(
                    "{}:{}:{}: {} [{}] {}",
                    diag.location.file.display(),
                    diag.location.line,
                    diag.location.column,
                    severity,
                    diag.rule_id,
                    diag.message
                );
            }
        }

        // Summary
        println!();
        println!("Lint Analysis");
        println!("  Tools: {}", tools_run.join(", "));
        println!("  Errors: {}", total_errors);
        println!("  Warnings: {}", total_warnings);

        if total_errors > 0 {
            println!();
            println!("Run 'moss lint --fix' to auto-fix issues where possible.");
        }
    }

    if total_errors > 0 {
        1
    } else {
        0
    }
}

/// Show callers/callees of a symbol
fn cmd_call_graph(
    root: &Path,
    target: &str,
    show_callers: bool,
    show_callees: bool,
    json: bool,
) -> i32 {
    // Try to parse target as file:symbol or just symbol
    let (symbol, file_hint) = if let Some((sym, file)) = parse_file_symbol_string(target) {
        (sym, Some(file))
    } else {
        (target.to_string(), None)
    };

    // Try index first
    let idx = match index::FileIndex::open_if_enabled(root) {
        Some(i) => i,
        None => {
            eprintln!("Indexing disabled or failed. Run: moss index rebuild --call-graph");
            return 1;
        }
    };

    let stats = idx.call_graph_stats().unwrap_or_default();
    if stats.calls == 0 {
        eprintln!("Call graph not indexed. Run: moss reindex --call-graph");
        return 1;
    }

    let mut results: Vec<(String, String, usize, &str)> = Vec::new(); // (file, symbol, line, direction)

    // Get callers if requested
    if show_callers {
        match idx.find_callers(&symbol) {
            Ok(callers) => {
                for (file, sym, line) in callers {
                    results.push((file, sym, line, "caller"));
                }
            }
            Err(e) => {
                eprintln!("Error finding callers: {}", e);
            }
        }
    }

    // Get callees if requested
    if show_callees {
        // Need to find file for symbol first
        let file_path = if let Some(f) = &file_hint {
            let matches = path_resolve::resolve(f, root);
            matches
                .iter()
                .find(|m| m.kind == "file")
                .map(|m| m.path.clone())
        } else {
            idx.find_symbol(&symbol)
                .ok()
                .and_then(|syms| syms.first().map(|(f, _, _, _)| f.clone()))
        };

        if let Some(file_path) = file_path {
            match idx.find_callees(&file_path, &symbol) {
                Ok(callees) => {
                    for (name, line) in callees {
                        results.push((file_path.clone(), name, line, "callee"));
                    }
                }
                Err(e) => {
                    eprintln!("Error finding callees: {}", e);
                }
            }
        }
    }

    if results.is_empty() {
        if json {
            println!("[]");
        } else {
            let direction = if show_callers && show_callees {
                "callers or callees"
            } else if show_callers {
                "callers"
            } else {
                "callees"
            };
            eprintln!("No {} found for: {}", direction, symbol);
        }
        return 1;
    }

    // Sort by file, then line
    results.sort_by(|a, b| (&a.0, a.2).cmp(&(&b.0, b.2)));

    if json {
        let output: Vec<_> = results
            .iter()
            .map(|(file, sym, line, direction)| {
                serde_json::json!({
                    "file": file,
                    "symbol": sym,
                    "line": line,
                    "direction": direction
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        let header = if show_callers && show_callees {
            format!("Callers and callees of {}", symbol)
        } else if show_callers {
            format!("Callers of {}", symbol)
        } else {
            format!("Callees of {}", symbol)
        };
        println!("{}:", header);
        for (file, sym, line, _direction) in &results {
            println!("  {}:{}:{}", file, line, sym);
        }
    }

    0
}

/// Try various separators to parse file:symbol format
fn parse_file_symbol_string(s: &str) -> Option<(String, String)> {
    // Try various separators: #, ::, :
    for sep in ["#", "::", ":"] {
        if let Some(idx) = s.find(sep) {
            let (file, rest) = s.split_at(idx);
            let symbol = &rest[sep.len()..];
            if !file.is_empty() && !symbol.is_empty() && looks_like_file(file) {
                return Some((symbol.to_string(), file.to_string()));
            }
        }
    }
    None
}

/// Check if a string looks like a file path
fn looks_like_file(s: &str) -> bool {
    s.contains('.') || s.contains('/')
}

/// Analyze codebase overview
fn cmd_overview(root: Option<&Path>, compact: bool, json: bool) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let report = overview::analyze_overview(&root);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "total_files": report.total_files,
                "files_by_language": report.files_by_language,
                "total_lines": report.total_lines,
                "total_functions": report.total_functions,
                "total_classes": report.total_classes,
                "total_methods": report.total_methods,
                "avg_complexity": (report.avg_complexity * 10.0).round() / 10.0,
                "max_complexity": report.max_complexity,
                "high_risk_functions": report.high_risk_functions,
                "functions_with_docs": report.functions_with_docs,
                "doc_coverage": (report.doc_coverage * 100.0).round() / 100.0,
                "total_imports": report.total_imports,
                "unique_modules": report.unique_modules,
                "todo_count": report.todo_count,
                "fixme_count": report.fixme_count,
                "health_score": (report.health_score * 100.0).round() / 100.0,
                "grade": report.grade
            })
        );
    } else if compact {
        println!("{}", report.format_compact());
    } else {
        println!("{}", report.format());
    }

    0
}

/// Hotspot data for a file
#[derive(Debug)]
struct FileHotspot {
    path: String,
    commits: usize,
    lines_added: usize,
    lines_deleted: usize,
    score: f64,
}

/// Analyze git history hotspots
fn cmd_hotspots(root: &Path, json: bool) -> i32 {
    // Check if git repo
    let git_dir = root.join(".git");
    if !git_dir.exists() {
        eprintln!("Not a git repository");
        return 1;
    }

    // Get file commit counts and churn from git log
    let output = match std::process::Command::new("git")
        .args(["log", "--format=", "--numstat"])
        .current_dir(root)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to run git log: {}", e);
            return 1;
        }
    };

    if !output.status.success() {
        eprintln!("git log failed");
        return 1;
    }

    // Parse numstat output: added<TAB>deleted<TAB>path
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_stats: std::collections::HashMap<String, (usize, usize, usize)> =
        std::collections::HashMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 3 {
            let added = parts[0].parse::<usize>().unwrap_or(0);
            let deleted = parts[1].parse::<usize>().unwrap_or(0);
            let path = parts[2].to_string();

            // Skip binary files (shown as -)
            if parts[0] == "-" || parts[1] == "-" {
                continue;
            }

            let entry = file_stats.entry(path).or_insert((0, 0, 0));
            entry.0 += 1; // commits
            entry.1 += added;
            entry.2 += deleted;
        }
    }

    // Get complexity from index
    let idx = match index::FileIndex::open_if_enabled(root) {
        Some(i) => i,
        None => {
            // No index, just use churn data
            let mut hotspots: Vec<FileHotspot> = file_stats
                .into_iter()
                .filter(|(path, _)| {
                    // Filter to source files only
                    let p = Path::new(path);
                    p.exists() && is_source_file(p)
                })
                .map(|(path, (commits, added, deleted))| {
                    let churn = added + deleted;
                    FileHotspot {
                        path,
                        commits,
                        lines_added: added,
                        lines_deleted: deleted,
                        score: (commits as f64) * (churn as f64).sqrt(),
                    }
                })
                .collect();

            hotspots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
            hotspots.truncate(20);

            return print_hotspots(&hotspots, json);
        }
    };

    // Build hotspots from churn data (index is available but not used for complexity)
    let _ = idx; // Index available for future on-demand complexity computation
    let mut hotspots: Vec<FileHotspot> = Vec::new();

    for (path, (commits, added, deleted)) in file_stats {
        let p = Path::new(&path);
        if !p.exists() || !is_source_file(p) {
            continue;
        }

        let churn = added + deleted;
        // Score: commits * sqrt(churn)
        let score = (commits as f64) * (churn as f64).sqrt();

        hotspots.push(FileHotspot {
            path,
            commits,
            lines_added: added,
            lines_deleted: deleted,
            score,
        });
    }

    hotspots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    hotspots.truncate(20);

    print_hotspots(&hotspots, json)
}

/// Check if a path is a source file we care about
fn is_source_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext,
            "rs" | "py"
                | "js"
                | "ts"
                | "tsx"
                | "jsx"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "rb"
                | "php"
                | "swift"
                | "kt"
                | "scala"
                | "cs"
                | "ex"
                | "exs"
        ),
        None => false,
    }
}

/// Print hotspots report
fn print_hotspots(hotspots: &[FileHotspot], json: bool) -> i32 {
    if hotspots.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No hotspots found (no git history or source files)");
        }
        return 0;
    }

    if json {
        let output: Vec<_> = hotspots
            .iter()
            .map(|h| {
                serde_json::json!({
                    "path": h.path,
                    "commits": h.commits,
                    "lines_added": h.lines_added,
                    "lines_deleted": h.lines_deleted,
                    "churn": h.lines_added + h.lines_deleted,
                    "score": (h.score * 10.0).round() / 10.0,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Git Hotspots (high churn)");
        println!();
        println!(
            "{:<50} {:>8} {:>8} {:>8}",
            "File", "Commits", "Churn", "Score"
        );
        println!("{}", "-".repeat(80));

        for h in hotspots {
            let churn = h.lines_added + h.lines_deleted;
            let display_path = if h.path.len() > 48 {
                format!("...{}", &h.path[h.path.len() - 45..])
            } else {
                h.path.clone()
            };
            println!(
                "{:<50} {:>8} {:>8} {:>8.0}",
                display_path, h.commits, churn, h.score
            );
        }

        println!();
        println!("Score = commits × √churn");
        println!("High scores indicate bug-prone files that change often.");
    }

    0
}

/// A broken reference found in documentation
#[derive(Debug)]
struct BrokenRef {
    file: String,
    line: usize,
    reference: String,
    context: String,
}

/// Check documentation references for broken links
fn cmd_check_refs(root: &Path, json: bool) -> i32 {
    use regex::Regex;

    // Open index to get known symbols
    let idx = match index::FileIndex::open_if_enabled(root) {
        Some(i) => i,
        None => {
            eprintln!("Indexing disabled or failed. Run: moss index rebuild --call-graph");
            return 1;
        }
    };

    // Get all symbol names from index
    let all_symbols = idx.all_symbol_names().unwrap_or_default();

    if all_symbols.is_empty() {
        eprintln!("No symbols indexed. Run: moss index rebuild --call-graph");
        return 1;
    }

    // Find markdown files
    let md_files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
                && !e
                    .path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if md_files.is_empty() {
        if json {
            println!(
                "{{\"broken_refs\": [], \"files_checked\": 0, \"symbols_indexed\": {}}}",
                all_symbols.len()
            );
        } else {
            println!("No markdown files found to check.");
        }
        return 0;
    }

    // Regex for code references: `identifier` or `Module::method` or `Module.method`
    let code_ref_re =
        Regex::new(r"`([A-Z][a-zA-Z0-9_]*(?:[:\.][a-zA-Z_][a-zA-Z0-9_]*)*)`").unwrap();

    let mut broken_refs: Vec<BrokenRef> = Vec::new();

    for md_file in &md_files {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = md_file
            .strip_prefix(root)
            .unwrap_or(md_file)
            .display()
            .to_string();

        for (line_num, line) in content.lines().enumerate() {
            for cap in code_ref_re.captures_iter(line) {
                let reference = &cap[1];

                // Extract symbol name (last part after :: or .)
                let symbol_name = reference
                    .rsplit(|c| c == ':' || c == '.')
                    .next()
                    .unwrap_or(reference);

                // Skip common non-symbol patterns
                if is_common_non_symbol(symbol_name) {
                    continue;
                }

                // Check if symbol exists
                if !all_symbols.contains(symbol_name) {
                    // Also check the full reference
                    let full_name = reference.replace("::", ".").replace(".", "::");
                    if !all_symbols.contains(&full_name) && !all_symbols.contains(reference) {
                        broken_refs.push(BrokenRef {
                            file: rel_path.clone(),
                            line: line_num + 1,
                            reference: reference.to_string(),
                            context: line.trim().to_string(),
                        });
                    }
                }
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "broken_refs": broken_refs.iter().map(|r| {
                serde_json::json!({
                    "file": r.file,
                    "line": r.line,
                    "reference": r.reference,
                    "context": r.context,
                })
            }).collect::<Vec<_>>(),
            "files_checked": md_files.len(),
            "symbols_indexed": all_symbols.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Documentation Reference Check");
        println!();
        println!("Files checked: {}", md_files.len());
        println!("Symbols indexed: {}", all_symbols.len());
        println!();

        if broken_refs.is_empty() {
            println!("No broken references found.");
        } else {
            println!("Broken references ({}):", broken_refs.len());
            println!();
            for r in &broken_refs {
                println!("  {}:{}: `{}`", r.file, r.line, r.reference);
                if r.context.len() <= 80 {
                    println!("    {}", r.context);
                }
            }
        }
    }

    if broken_refs.is_empty() {
        0
    } else {
        1
    }
}

/// Check if a string is a common non-symbol pattern (command, path, etc.)
fn is_common_non_symbol(s: &str) -> bool {
    // Skip common patterns that aren't symbols
    matches!(
        s,
        "TODO"
            | "FIXME"
            | "NOTE"
            | "HACK"
            | "XXX"
            | "BUG"
            | "OK"
            | "Err"
            | "Ok"
            | "None"
            | "Some"
            | "True"
            | "False"
            | "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arc"
            | "Rc"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "PathBuf"
            | "Path"
            | "File"
            | "Read"
            | "Write"
            | "Debug"
            | "Clone"
            | "Copy"
            | "Default"
            | "Send"
            | "Sync"
            | "Serialize"
            | "Deserialize"
    ) || s.len() < 2
        || s.chars().all(|c| c.is_uppercase() || c == '_') // ALL_CAPS constants
}

/// A doc file with stale code coverage
#[derive(Debug)]
struct StaleDoc {
    doc_path: String,
    doc_modified: u64,
    stale_covers: Vec<StaleCover>,
}

/// A stale coverage declaration
#[derive(Debug)]
struct StaleCover {
    pattern: String,
    code_modified: u64,
    matching_files: Vec<String>,
}

/// Find docs with stale code coverage
fn cmd_stale_docs(root: &Path, json: bool) -> i32 {
    use regex::Regex;

    // Find markdown files with <!-- covers: ... --> declarations
    let covers_re = Regex::new(r"<!--\s*covers:\s*(.+?)\s*-->").unwrap();

    let md_files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
                && !e
                    .path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if md_files.is_empty() {
        if json {
            println!("{{\"stale_docs\": [], \"files_checked\": 0}}");
        } else {
            println!("No markdown files found.");
        }
        return 0;
    }

    let mut stale_docs: Vec<StaleDoc> = Vec::new();
    let mut files_with_covers = 0;

    for md_file in &md_files {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Find all covers declarations
        let covers: Vec<String> = covers_re
            .captures_iter(&content)
            .map(|cap| cap[1].to_string())
            .collect();

        if covers.is_empty() {
            continue;
        }

        files_with_covers += 1;

        let rel_path = md_file
            .strip_prefix(root)
            .unwrap_or(md_file)
            .display()
            .to_string();

        // Get doc modification time
        let doc_modified = std::fs::metadata(md_file)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
            .unwrap_or(0);

        let mut stale_covers: Vec<StaleCover> = Vec::new();

        for cover_pattern in covers {
            // Parse comma-separated patterns
            for pattern in cover_pattern.split(',').map(|s| s.trim()) {
                if pattern.is_empty() {
                    continue;
                }

                // Find matching files using glob
                let matching = find_covered_files(root, pattern);

                if matching.is_empty() {
                    continue;
                }

                // Check if any matching file was modified after the doc
                let code_modified = matching
                    .iter()
                    .filter_map(|f| {
                        std::fs::metadata(root.join(f))
                            .and_then(|m| m.modified())
                            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
                            .ok()
                    })
                    .max()
                    .unwrap_or(0);

                if code_modified > doc_modified {
                    stale_covers.push(StaleCover {
                        pattern: pattern.to_string(),
                        code_modified,
                        matching_files: matching,
                    });
                }
            }
        }

        if !stale_covers.is_empty() {
            stale_docs.push(StaleDoc {
                doc_path: rel_path,
                doc_modified,
                stale_covers,
            });
        }
    }

    if json {
        let output = serde_json::json!({
            "stale_docs": stale_docs.iter().map(|d| {
                serde_json::json!({
                    "doc": d.doc_path,
                    "doc_modified": d.doc_modified,
                    "stale_covers": d.stale_covers.iter().map(|c| {
                        serde_json::json!({
                            "pattern": c.pattern,
                            "code_modified": c.code_modified,
                            "files": c.matching_files,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "files_checked": md_files.len(),
            "files_with_covers": files_with_covers,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Stale Documentation Check");
        println!();
        println!("Files checked: {}", md_files.len());
        println!("Files with covers: {}", files_with_covers);
        println!();

        if stale_docs.is_empty() {
            println!("No stale docs found. All covered code is older than docs.");
        } else {
            println!("Stale docs ({}):", stale_docs.len());
            println!();
            for doc in &stale_docs {
                println!("  {}", doc.doc_path);
                for cover in &doc.stale_covers {
                    let days_stale = (cover.code_modified - doc.doc_modified) / 86400;
                    println!(
                        "    {} ({} files, ~{} days stale)",
                        cover.pattern,
                        cover.matching_files.len(),
                        days_stale
                    );
                }
            }
        }
    }

    if stale_docs.is_empty() {
        0
    } else {
        1
    }
}

/// Find files matching a cover pattern (glob or path prefix)
fn find_covered_files(root: &Path, pattern: &str) -> Vec<String> {
    // Check if it's a glob pattern
    if pattern.contains('*') {
        // Use glob matching
        let full_pattern = root.join(pattern);
        glob::glob(full_pattern.to_str().unwrap_or(""))
            .ok()
            .map(|paths| {
                paths
                    .filter_map(|p| p.ok())
                    .filter(|p| p.is_file())
                    .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.display().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        // Treat as exact path or prefix
        let target = root.join(pattern);
        if target.is_file() {
            vec![pattern.to_string()]
        } else if target.is_dir() {
            // Find all files in directory
            walkdir::WalkDir::new(&target)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    e.path()
                        .strip_prefix(root)
                        .ok()
                        .map(|r| r.display().to_string())
                })
                .collect()
        } else {
            vec![]
        }
    }
}

/// A missing example reference
#[derive(Debug)]
struct MissingExample {
    doc_file: String,
    line: usize,
    reference: String, // path#name
}

/// Check that all example references have matching markers
fn cmd_check_examples(root: &Path, json: bool) -> i32 {
    use regex::Regex;
    use std::collections::HashSet;

    // Find all example markers in source files: // [example: name] ... // [/example]
    let marker_start_re = Regex::new(r"//\s*\[example:\s*([^\]]+)\]").unwrap();

    // Find all example references in docs: {{example: path#name}}
    let ref_re = Regex::new(r"\{\{example:\s*([^}]+)\}\}").unwrap();

    // Collect all defined examples: (file, name)
    let mut defined_examples: HashSet<String> = HashSet::new();

    // Walk source files
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && !path
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
    {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        // Only check source files (where we'd have // [example:] markers)
        if !matches!(
            ext,
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c" | "cpp" | "rb"
        ) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        for cap in marker_start_re.captures_iter(&content) {
            let name = cap[1].trim();
            // Key: path#name
            let key = format!("{}#{}", rel_path, name);
            defined_examples.insert(key);
        }
    }

    // Find all references in markdown files
    let mut missing: Vec<MissingExample> = Vec::new();
    let mut refs_found = 0;

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
                && !e
                    .path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let mut in_code_block = false;
        for (line_num, line) in content.lines().enumerate() {
            // Track fenced code blocks
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }

            for cap in ref_re.captures_iter(line) {
                // Skip if match is inside backticks (inline code)
                let match_start = cap.get(0).unwrap().start();
                let match_end = cap.get(0).unwrap().end();
                let before = &line[..match_start];
                let after = &line[match_end..];

                // Count backticks before match - odd count means we're inside inline code
                if before.chars().filter(|&c| c == '`').count() % 2 == 1 && after.contains('`') {
                    continue;
                }

                refs_found += 1;
                let reference = cap[1].trim();

                // Reference should be path#name
                if !defined_examples.contains(reference) {
                    missing.push(MissingExample {
                        doc_file: rel_path.clone(),
                        line: line_num + 1,
                        reference: reference.to_string(),
                    });
                }
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "defined_examples": defined_examples.len(),
            "references_found": refs_found,
            "missing": missing.iter().map(|m| {
                serde_json::json!({
                    "doc": m.doc_file,
                    "line": m.line,
                    "reference": m.reference,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Example Reference Check");
        println!();
        println!("Defined examples: {}", defined_examples.len());
        println!("References found: {}", refs_found);
        println!();

        if missing.is_empty() {
            println!("All example references are valid.");
        } else {
            println!("Missing examples ({}):", missing.len());
            println!();
            for m in &missing {
                println!("  {}:{}: {{{{{}}}}}", m.doc_file, m.line, m.reference);
            }
        }
    }

    if missing.is_empty() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_duplicate_functions_allowlist_empty() {
        let tmp = tempdir().unwrap();
        let allowlist = load_duplicate_functions_allowlist(tmp.path());
        assert!(allowlist.is_empty());
    }

    #[test]
    fn test_load_duplicate_functions_allowlist_with_entries() {
        let tmp = tempdir().unwrap();
        let moss_dir = tmp.path().join(".moss");
        fs::create_dir_all(&moss_dir).unwrap();
        fs::write(
            moss_dir.join("duplicate-functions-allow"),
            "# Comment\nsrc/foo.rs:bar\nsrc/baz.rs:qux\n",
        )
        .unwrap();

        let allowlist = load_duplicate_functions_allowlist(tmp.path());
        assert_eq!(allowlist.len(), 2);
        assert!(allowlist.contains("src/foo.rs:bar"));
        assert!(allowlist.contains("src/baz.rs:qux"));
    }

    #[test]
    fn test_load_duplicate_functions_allowlist_ignores_comments() {
        let tmp = tempdir().unwrap();
        let moss_dir = tmp.path().join(".moss");
        fs::create_dir_all(&moss_dir).unwrap();
        fs::write(
            moss_dir.join("duplicate-functions-allow"),
            "# This is a comment\n# Another comment\nsrc/foo.rs:bar\n",
        )
        .unwrap();

        let allowlist = load_duplicate_functions_allowlist(tmp.path());
        assert_eq!(allowlist.len(), 1);
        assert!(allowlist.contains("src/foo.rs:bar"));
    }

    /// Helper to check if a duplicate function group is fully allowed
    fn is_group_allowed(
        locations: &[DuplicateFunctionLocation],
        allowlist: &std::collections::HashSet<String>,
    ) -> bool {
        locations
            .iter()
            .all(|loc| allowlist.contains(&format!("{}:{}", loc.file, loc.symbol)))
    }

    #[test]
    fn test_is_group_allowed_all_in_allowlist() {
        let mut allowlist = std::collections::HashSet::new();
        allowlist.insert("src/a.rs:foo".to_string());
        allowlist.insert("src/b.rs:bar".to_string());

        let locations = vec![
            DuplicateFunctionLocation {
                file: "src/a.rs".to_string(),
                symbol: "foo".to_string(),
                start_line: 1,
                end_line: 5,
            },
            DuplicateFunctionLocation {
                file: "src/b.rs".to_string(),
                symbol: "bar".to_string(),
                start_line: 10,
                end_line: 15,
            },
        ];

        assert!(is_group_allowed(&locations, &allowlist));
    }

    #[test]
    fn test_is_group_allowed_partial_not_allowed() {
        let mut allowlist = std::collections::HashSet::new();
        allowlist.insert("src/a.rs:foo".to_string());

        let locations = vec![
            DuplicateFunctionLocation {
                file: "src/a.rs".to_string(),
                symbol: "foo".to_string(),
                start_line: 1,
                end_line: 5,
            },
            DuplicateFunctionLocation {
                file: "src/b.rs".to_string(),
                symbol: "bar".to_string(),
                start_line: 10,
                end_line: 15,
            },
        ];

        assert!(!is_group_allowed(&locations, &allowlist));
    }

    #[test]
    fn test_calculate_grade_perfect() {
        // (score, weight) pairs - all 100%
        let scores = [(100.0, 1.0), (100.0, 0.5), (100.0, 2.0), (100.0, 0.3)];
        let (letter, percentage) = calculate_grade(&scores);
        assert_eq!(letter, "A");
        assert!((percentage - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_calculate_grade_weights() {
        // Security weight is 2.0, so a security issue hurts more than complexity
        // 50% health (weight 1.0), 100% complexity (weight 0.5), 0% security (weight 2.0), 100% duplicate-functions
        let scores = [(50.0, 1.0), (100.0, 0.5), (0.0, 2.0), (100.0, 0.3)];
        let (_, percentage) = calculate_grade(&scores);
        // Expected: (50*1 + 100*0.5 + 0*2 + 100*0.3) / (1+0.5+2+0.3) = 130/3.8 ≈ 34.2%
        assert!(percentage < 50.0); // Security weight should drag it down
    }
}
