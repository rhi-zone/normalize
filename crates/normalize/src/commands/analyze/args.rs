//! Analyze command arguments with subcommands

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Helper for serde default = true
fn default_true() -> bool {
    true
}

/// Analyze command arguments.
#[derive(Args, Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeArgs {
    #[command(subcommand)]
    pub command: Option<AnalyzeCommand>,

    /// Root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    pub root: Option<PathBuf>,

    /// Exclude paths matching pattern or @alias
    #[arg(long, value_name = "PATTERN", value_delimiter = ',', global = true)]
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Include only paths matching pattern or @alias
    #[arg(long, value_name = "PATTERN", value_delimiter = ',', global = true)]
    #[serde(default)]
    pub only: Vec<String>,

    /// Analyze only files changed since base ref (e.g., main, HEAD~1)
    /// If no BASE given, defaults to origin's default branch
    #[arg(long, value_name = "BASE", global = true, num_args = 0..=1, default_missing_value = "")]
    pub diff: Option<String>,
}

#[derive(Subcommand, Debug, serde::Deserialize, schemars::JsonSchema)]
pub enum AnalyzeCommand {
    /// Run health analysis (file counts, complexity stats, large file warnings)
    Health {
        /// Target file or directory
        target: Option<String>,
    },

    /// Analyze codebase architecture: coupling, cycles, dependencies
    Architecture,

    /// Run complexity analysis
    Complexity {
        /// Target file or directory
        target: Option<String>,

        /// Only show functions above this threshold
        #[arg(short, long)]
        threshold: Option<usize>,

        /// Maximum number of functions to show (0 = no limit)
        #[arg(short = 'l', long, default_value = "10")]
        limit: usize,

        /// Filter by symbol kind: function, method
        #[arg(long)]
        kind: Option<String>,

        /// Output in SARIF format for IDE integration
        #[arg(long)]
        #[serde(default)]
        sarif: bool,

        /// Add function to .normalize/complexity-allow
        #[arg(long, value_name = "SYMBOL")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Run function length analysis
    Length {
        /// Target file or directory
        target: Option<String>,

        /// Output in SARIF format for IDE integration
        #[arg(long)]
        #[serde(default)]
        sarif: bool,

        /// Add function to .normalize/length-allow
        #[arg(long, value_name = "SYMBOL")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Run security analysis
    Security {
        /// Target file or directory
        target: Option<String>,
    },

    /// Analyze documentation coverage
    Docs {
        /// Number of worst-covered files to show
        #[arg(short = 'l', long, default_value = "10")]
        limit: usize,
    },

    /// Show longest files in codebase
    Files {
        /// Number of files to show
        #[arg(short = 'l', long, default_value = "20")]
        limit: usize,

        /// Add pattern to .normalize/large-files-allow
        #[arg(long, value_name = "PATTERN")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Trace value provenance for a symbol
    Trace {
        /// Symbol to trace (format: symbol or file:line or file/symbol)
        symbol: String,

        /// Target file to search in
        #[arg(long)]
        target: Option<String>,

        /// Maximum trace depth
        #[arg(long, default_value = "10")]
        max_depth: usize,

        /// Trace into called functions (show what they return)
        #[arg(long)]
        #[serde(default)]
        recursive: bool,

        /// Case-insensitive symbol matching
        #[arg(short = 'i', long)]
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Show what functions call a symbol
    Callers {
        /// Symbol to find callers for
        symbol: String,

        /// Case-insensitive symbol matching
        #[arg(short = 'i', long)]
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Show what functions a symbol calls
    Callees {
        /// Symbol to find callees for
        symbol: String,

        /// Case-insensitive symbol matching
        #[arg(short = 'i', long)]
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Show git history hotspots (frequently changed files)
    Hotspots {
        /// Add pattern to .normalize/hotspots-allow
        #[arg(long, value_name = "PATTERN")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Check documentation references for broken links
    CheckRefs,

    /// Find documentation with stale code references
    StaleDocs,

    /// Check example references in documentation
    CheckExamples,

    /// Detect duplicate functions (code clones)
    DuplicateFunctions {
        /// Elide identifier names when comparing (default: true)
        #[arg(long, default_value = "true")]
        #[serde(default = "default_true")]
        elide_identifiers: bool,

        /// Elide literal values when comparing
        #[arg(long)]
        #[serde(default)]
        elide_literals: bool,

        /// Show source code for detected duplicates
        #[arg(long)]
        #[serde(default)]
        show_source: bool,

        /// Minimum lines for a function to be considered
        #[arg(long, default_value = "1")]
        min_lines: usize,

        /// Include groups where all functions share the same name (likely trait implementations).
        /// By default these are suppressed as intentionally parallel, not copy-paste.
        #[arg(long)]
        #[serde(default)]
        include_trait_impls: bool,

        /// Allow a duplicate function group (add to .normalize/duplicate-functions-allow)
        /// Accepts file:symbol (e.g., src/foo.rs:my_func) or file:start-end (e.g., src/foo.rs:10-20)
        #[arg(long, value_name = "LOCATION")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Detect duplicate code blocks (subtree-level clone detection)
    DuplicateBlocks {
        #[arg(long, default_value = "true")]
        #[serde(default = "default_true")]
        elide_identifiers: bool,

        #[arg(long)]
        #[serde(default)]
        elide_literals: bool,

        #[arg(long)]
        #[serde(default)]
        show_source: bool,

        /// Minimum lines for a block to be considered [default: 5]
        #[arg(long, default_value = "5")]
        min_lines: usize,

        /// Skip function/method nodes (avoid overlap with duplicate-functions)
        #[arg(long)]
        #[serde(default)]
        skip_functions: bool,

        /// Allow a duplicate block group (add to .normalize/duplicate-blocks-allow)
        /// Accepts file:func:start-end or file:start-end
        #[arg(long, value_name = "LOCATION")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Detect similar (fuzzy-matching) functions via MinHash LSH
    SimilarFunctions {
        #[arg(long, default_value = "true")]
        #[serde(default = "default_true")]
        elide_identifiers: bool,

        #[arg(long)]
        #[serde(default)]
        elide_literals: bool,

        #[arg(long)]
        #[serde(default)]
        show_source: bool,

        /// Minimum lines for a function to be considered [default: 10]
        #[arg(long, default_value = "10")]
        min_lines: usize,

        /// Minimum similarity threshold (0.0–1.0) [default: 0.85]
        #[arg(long, default_value = "0.85")]
        similarity: f64,

        /// Skeleton mode: match on control-flow structure, ignoring body content
        #[arg(long)]
        #[serde(default)]
        skeleton: bool,

        /// Include pairs where both functions share the same name (likely trait implementations).
        /// By default these are suppressed as intentionally parallel, not copy-paste.
        #[arg(long)]
        #[serde(default)]
        include_trait_impls: bool,

        /// Allow a similar function pair (add to .normalize/similar-functions-allow)
        /// Accepts file:symbol:start-end or file:start-end
        #[arg(long, value_name = "LOCATION")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Detect similar (fuzzy-matching) code blocks via MinHash LSH
    SimilarBlocks {
        #[arg(long, default_value = "true")]
        #[serde(default = "default_true")]
        elide_identifiers: bool,

        #[arg(long)]
        #[serde(default)]
        elide_literals: bool,

        #[arg(long)]
        #[serde(default)]
        show_source: bool,

        /// Minimum lines for a block to be considered [default: 10]
        #[arg(long, default_value = "10")]
        min_lines: usize,

        /// Minimum similarity threshold (0.0–1.0) [default: 0.85]
        #[arg(long, default_value = "0.85")]
        similarity: f64,

        /// Skeleton mode: replace block/body subtrees with a placeholder, matching
        /// on control-flow structure regardless of body content or size
        #[arg(long)]
        #[serde(default)]
        skeleton: bool,

        /// Include pairs where both blocks are inside same-name functions (likely trait implementations).
        /// By default these are suppressed as intentionally parallel, not copy-paste.
        #[arg(long)]
        #[serde(default)]
        include_trait_impls: bool,

        /// Allow a similar block pair (add to .normalize/similar-blocks-allow)
        /// Accepts file:func:start-end or file:start-end
        #[arg(long, value_name = "LOCATION")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Detect duplicate type definitions
    DuplicateTypes {
        /// Target directory to scan
        target: Option<String>,

        /// Minimum field overlap percentage (default: 70)
        #[arg(long, default_value = "70")]
        min_overlap: usize,

        /// Allow a duplicate type pair (add to .normalize/duplicate-types-allow)
        #[arg(long, num_args = 2, value_names = ["TYPE1", "TYPE2"])]
        allow: Option<Vec<String>>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Find public functions with no direct test caller
    TestGaps {
        /// Target file or directory
        target: Option<String>,

        /// Show all functions (including tested), sorted by test calls ascending
        #[arg(long)]
        #[serde(default)]
        all: bool,

        /// Only show functions above this risk threshold
        #[arg(long, value_name = "N")]
        min_risk: Option<f64>,

        /// Maximum number of functions to show (0 = no limit)
        #[arg(short = 'l', long, default_value = "20")]
        limit: usize,

        /// Output in SARIF format for IDE integration
        #[arg(long)]
        #[serde(default)]
        sarif: bool,

        /// Add function to .normalize/test-gaps-allow
        #[arg(long, value_name = "SYMBOL")]
        allow: Option<String>,

        /// Reason for allowing
        #[arg(long)]
        reason: Option<String>,
    },

    /// Run all analysis passes
    All {
        /// Target file or directory
        target: Option<String>,
    },

    /// Show AST for a file (for authoring syntax rules)
    Ast {
        /// File to parse
        file: PathBuf,

        /// Show AST node at this line number
        #[arg(long)]
        at: Option<usize>,

        /// Output as S-expression (default: tree format)
        #[arg(long)]
        #[serde(default)]
        sexp: bool,
    },

    /// Test a tree-sitter query against files
    ///
    /// Supports two pattern syntaxes (auto-detected):
    /// - Tree-sitter S-expression: (call_expression function: (identifier) @fn)
    /// - ast-grep pattern: $FN($ARGS)
    Query {
        /// Pattern to search for (tree-sitter S-expr or ast-grep pattern)
        pattern: String,

        /// File or directory to search (searches all files if omitted)
        path: Option<PathBuf>,

        /// Show full matched source code
        #[arg(long)]
        #[serde(default)]
        show_source: bool,

        /// Lines of context to show in preview
        #[arg(short = 'C', long)]
        context: Option<usize>,
    },

    /// Run syntax rules from .normalize/rules/*.scm
    Rules {
        /// Run only this specific rule
        #[arg(long)]
        rule: Option<String>,

        /// List available rules without running them
        #[arg(long)]
        #[serde(default)]
        list: bool,

        /// Auto-fix issues that have fixes available
        #[arg(long)]
        #[serde(default)]
        fix: bool,

        /// Output in SARIF format for IDE integration
        #[arg(long)]
        #[serde(default)]
        sarif: bool,

        /// Target directory to scan
        target: Option<String>,

        /// Enable debug output (comma-delimited: timing, all)
        #[arg(long, value_delimiter = ',')]
        #[serde(default)]
        debug: Vec<String>,
    },
}
