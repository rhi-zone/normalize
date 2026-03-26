//! Native rule checks for normalize.
//!
//! Implements stale-summary, check-refs, stale-docs, check-examples, ratchet, and budget as
//! pure Rust checks (no tree-sitter AST parsing). These are the "native engine"
//! checks invoked by `normalize rules run --engine native`.

pub mod budget;
pub mod check_examples;
pub mod check_refs;
pub mod ratchet;
pub mod scm_capture_names;
pub mod stale_docs;
pub mod stale_summary;
pub(crate) mod walk;

pub use budget::{BudgetRulesReport, build_budget_report};
pub use check_examples::build_check_examples_report;
pub use check_refs::build_check_refs_report;
pub use ratchet::{RatchetRulesReport, build_ratchet_report};
pub use scm_capture_names::build_scm_capture_names_report;
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
}

/// All native rules with their default metadata.
pub const NATIVE_RULES: &[NativeRuleDescriptor] = &[
    NativeRuleDescriptor {
        id: "broken-ref",
        default_severity: "warning",
        message: "Backtick reference in docs/comments doesn't resolve to a known symbol or file",
        tags: &["correctness", "documentation"],
    },
    NativeRuleDescriptor {
        id: "missing-summary",
        default_severity: "error",
        message: "Directory is missing a required doc file (default: SUMMARY.md; configurable via filenames and paths)",
        tags: &["documentation"],
    },
    NativeRuleDescriptor {
        id: "stale-summary",
        default_severity: "error",
        message: "Doc file hasn't been updated since files in the directory changed (default: SUMMARY.md; configurable via filenames and paths)",
        tags: &["documentation"],
    },
    NativeRuleDescriptor {
        id: "stale-doc",
        default_severity: "info",
        message: "Doc comment references a symbol that no longer exists",
        tags: &["documentation"],
    },
    NativeRuleDescriptor {
        id: "missing-example",
        default_severity: "warning",
        message: "Example referenced in docs doesn't appear in the source file",
        tags: &["documentation"],
    },
    NativeRuleDescriptor {
        id: "ratchet/complexity",
        default_severity: "error",
        message: "Cyclomatic complexity has regressed past the ratchet baseline",
        tags: &["quality", "complexity"],
    },
    NativeRuleDescriptor {
        id: "ratchet/call-complexity",
        default_severity: "error",
        message: "Transitive call complexity has regressed past the ratchet baseline",
        tags: &["quality", "complexity"],
    },
    NativeRuleDescriptor {
        id: "ratchet/line-count",
        default_severity: "error",
        message: "File line count has regressed past the ratchet baseline",
        tags: &["quality"],
    },
    NativeRuleDescriptor {
        id: "ratchet/function-count",
        default_severity: "error",
        message: "Function count has regressed past the ratchet baseline",
        tags: &["quality"],
    },
    NativeRuleDescriptor {
        id: "ratchet/class-count",
        default_severity: "error",
        message: "Class count has regressed past the ratchet baseline",
        tags: &["quality"],
    },
    NativeRuleDescriptor {
        id: "ratchet/comment-line-count",
        default_severity: "error",
        message: "Comment line count has regressed past the ratchet baseline",
        tags: &["quality"],
    },
    NativeRuleDescriptor {
        id: "budget/lines",
        default_severity: "error",
        message: "Line diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "budget/functions",
        default_severity: "error",
        message: "Function diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "budget/classes",
        default_severity: "error",
        message: "Class diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "budget/modules",
        default_severity: "error",
        message: "Module diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "budget/todos",
        default_severity: "error",
        message: "TODO/FIXME diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "budget/complexity-delta",
        default_severity: "error",
        message: "Complexity delta exceeds configured budget limit",
        tags: &["quality", "budget", "complexity"],
    },
    NativeRuleDescriptor {
        id: "budget/dependencies",
        default_severity: "error",
        message: "Dependency diff exceeds configured budget limit",
        tags: &["quality", "budget"],
    },
    NativeRuleDescriptor {
        id: "scm-capture-names",
        default_severity: "warning",
        message: "Unexpected capture name in a .calls.scm query file (not consumed by the facts system)",
        tags: &["correctness", "query"],
    },
];
