//! Native rule checks for normalize.
//!
//! Implements stale-summary, check-refs, stale-docs, and check-examples as
//! pure Rust checks (no tree-sitter AST parsing). These are the "native engine"
//! checks invoked by `normalize rules run --engine native` and
//! `normalize analyze check`.

pub mod check_examples;
pub mod check_refs;
pub mod stale_docs;
pub mod stale_summary;
pub(crate) mod walk;

pub use check_examples::build_check_examples_report;
pub use check_refs::build_check_refs_report;
pub use stale_docs::build_stale_docs_report;
pub use stale_summary::build_stale_summary_report;
