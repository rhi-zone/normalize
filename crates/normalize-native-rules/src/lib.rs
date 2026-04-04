//! Native rule checks for normalize.
//!
//! Implements stale-summary, check-refs, stale-docs, check-examples, ratchet, budget,
//! long-file, high-complexity, and long-function as pure Rust checks. These are the
//! "native engine" checks invoked by `normalize rules run --engine native`.

pub mod budget;
pub mod cache;
pub mod check_examples;
pub mod check_refs;
pub mod high_complexity;
pub mod long_file;
pub mod long_function;
pub mod ratchet;
pub mod stale_doc;
pub mod stale_docs;
pub mod stale_summary;
pub mod walk;

pub use cache::{FindingsCache, file_mtime_nanos as cache_file_mtime_nanos};

pub use budget::{BudgetRulesReport, build_budget_report};
pub use check_examples::build_check_examples_report;
pub use check_refs::build_check_refs_report;
pub use high_complexity::build_high_complexity_report;
pub use long_file::build_long_file_report;
pub use long_function::build_long_function_report;
pub use ratchet::{RatchetRulesReport, build_ratchet_report};
pub use stale_doc::{StaleDocConfig, build_stale_doc_report};
pub use stale_docs::build_stale_docs_report;
pub use stale_summary::{build_missing_summary_report, build_stale_summary_report};

/// Static descriptor for a native rule's default metadata.
///
/// `default_severity` is the baked-in severity from the rule author. At runtime
/// the actual severity may differ: `normalize rules run` applies any
/// `[rules."rule-id"]` overrides from the project's `normalize.toml` via
/// `normalize_rules::apply_native_rules_config` before presenting findings to the user.
pub struct NativeRuleDescriptor {
    /// Unique rule identifier (e.g. `"stale-summary"`).
    pub id: &'static str,
    /// Default severity before any project-level override (`"error"`, `"warning"`, or `"info"`).
    pub default_severity: &'static str,
    /// Short human-readable description of what the rule checks.
    pub message: &'static str,
    /// Tags used for grouping and filtering (e.g. `&["docs", "quality"]`).
    pub tags: &'static [&'static str],
    /// Whether the rule is enabled by default (before any project-level override).
    /// Advisory rules like `long-file` default to `false`.
    pub default_enabled: bool,
}

/// All native rules with their default metadata.
pub const NATIVE_RULES: &[NativeRuleDescriptor] = &[
    NativeRuleDescriptor {
        id: "broken-ref",
        default_severity: "warning",
        message: "Backtick reference in docs/comments doesn't resolve to a known symbol or file",
        tags: &["correctness", "documentation"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "missing-summary",
        default_severity: "error",
        message: "Directory is missing a required doc file (default: SUMMARY.md; configurable via filenames and paths)",
        tags: &["documentation"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "stale-summary",
        default_severity: "error",
        message: "Doc file hasn't been updated since files in the directory changed (default: SUMMARY.md; configurable via filenames and paths)",
        tags: &["documentation"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "stale-doc",
        default_severity: "warning",
        message: "Documentation file may be stale — a strongly co-changed code file was updated more recently",
        tags: &["docs", "freshness"],
        default_enabled: false,
    },
    NativeRuleDescriptor {
        id: "missing-example",
        default_severity: "warning",
        message: "Example referenced in docs doesn't appear in the source file",
        tags: &["documentation"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/complexity",
        default_severity: "error",
        message: "Cyclomatic complexity has regressed past the ratchet baseline",
        tags: &["quality", "complexity"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/call-complexity",
        default_severity: "error",
        message: "Transitive call complexity has regressed past the ratchet baseline",
        tags: &["quality", "complexity"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/line-count",
        default_severity: "error",
        message: "File line count has regressed past the ratchet baseline",
        tags: &["quality"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/function-count",
        default_severity: "error",
        message: "Function count has regressed past the ratchet baseline",
        tags: &["quality"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/class-count",
        default_severity: "error",
        message: "Class count has regressed past the ratchet baseline",
        tags: &["quality"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "ratchet/comment-line-count",
        default_severity: "error",
        message: "Comment line count has regressed past the ratchet baseline",
        tags: &["quality"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/lines",
        default_severity: "error",
        message: "Line diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/functions",
        default_severity: "error",
        message: "Function diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/classes",
        default_severity: "error",
        message: "Class diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/modules",
        default_severity: "error",
        message: "Module diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/todos",
        default_severity: "error",
        message: "TODO/FIXME diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/complexity-delta",
        default_severity: "error",
        message: "Complexity delta exceeds configured budget limit",
        tags: &["quality", "budget", "complexity"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "budget/dependencies",
        default_severity: "error",
        message: "Dependency diff exceeds configured budget limit",
        tags: &["quality", "budget"],
        default_enabled: true,
    },
    NativeRuleDescriptor {
        id: "long-file",
        default_severity: "warning",
        message: "Source file exceeds line count threshold (default: 500 lines)",
        tags: &["quality"],
        default_enabled: false,
    },
    NativeRuleDescriptor {
        id: "high-complexity",
        default_severity: "warning",
        message: "Function exceeds cyclomatic complexity threshold (default: 20)",
        tags: &["quality", "complexity"],
        default_enabled: false,
    },
    NativeRuleDescriptor {
        id: "long-function",
        default_severity: "warning",
        message: "Function exceeds line count threshold (default: 100 lines)",
        tags: &["quality"],
        default_enabled: false,
    },
];
