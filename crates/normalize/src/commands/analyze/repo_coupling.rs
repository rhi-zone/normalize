//! Cross-repo coupling analysis — re-export shim.
//!
//! Typed compute and presentation both live in
//! `normalize_git_history::repo_coupling` (the `OutputFormatter` impl is gated
//! behind that crate's `cli` feature).

pub use normalize_git_history::repo_coupling::{
    DepEdge, RepoCouplingContext, RepoCouplingReport, TemporalCouplingPair, analyze_repo_coupling,
};
