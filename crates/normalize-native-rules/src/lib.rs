//! Native rule checks for normalize.
//!
//! Implements stale-summary, check-refs, stale-docs, check-examples, and ratchet as
//! pure Rust checks (no tree-sitter AST parsing). These are the "native engine"
//! checks invoked by `normalize rules run --engine native`.

pub mod check_examples;
pub mod check_refs;
pub mod ratchet;
pub mod stale_docs;
pub mod stale_summary;
pub(crate) mod walk;

pub use check_examples::build_check_examples_report;
pub use check_refs::build_check_refs_report;
pub use ratchet::build_ratchet_report;
pub use stale_docs::build_stale_docs_report;
pub use stale_summary::build_stale_summary_report;

/// Static descriptor for a native rule's default metadata.
pub struct NativeRuleDescriptor {
    pub id: &'static str,
    pub default_severity: &'static str,
    pub message: &'static str,
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
        default_severity: "warning",
        message: "Directory is missing a SUMMARY.md file",
        tags: &["documentation"],
    },
    NativeRuleDescriptor {
        id: "stale-summary",
        default_severity: "info",
        message: "SUMMARY.md hasn't been updated since files in the directory changed",
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
];
