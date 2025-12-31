//! Analyze command - run analysis on target.

mod call_graph;
mod check_examples;
mod check_refs;
mod duplicates;
mod health;
mod hotspots;
mod lint;
mod stale_docs;
mod trace;

use crate::analysis_report;
use crate::analyze::complexity::ComplexityReport;
use crate::commands::filter::detect_project_languages;
use crate::config::MossConfig;
use crate::daemon;
use crate::filter::Filter;
use crate::merge::Merge;
use clap::Args;
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
        return health::cmd_overview(root, compact, json);
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
        return call_graph::cmd_call_graph(&root, target, callers, callees, json);
    }

    // --lint runs linter analysis
    if lint {
        return lint::cmd_lint_analyze(&root, target, json);
    }

    // --hotspots runs git history hotspot analysis
    if hotspots {
        return hotspots::cmd_hotspots(&root, json);
    }

    // --check-refs validates documentation references
    if check_refs {
        return check_refs::cmd_check_refs(&root, json);
    }

    // --stale-docs finds docs where covered code has changed
    if stale_docs {
        return stale_docs::cmd_stale_docs(&root, json);
    }

    // --check-examples validates example references
    if check_examples {
        return check_examples::cmd_check_examples(&root, json);
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

/// Check if a path is a source file we care about
pub(crate) fn is_source_file(path: &Path) -> bool {
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
