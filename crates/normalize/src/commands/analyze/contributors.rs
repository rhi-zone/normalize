//! Cross-repo contributor analysis — re-export shim.
//!
//! Typed compute and presentation both live in
//! `normalize_git_history::contributors` (the `OutputFormatter` impl is gated
//! behind that crate's `cli` feature).

pub use normalize_git_history::contributors::{
    ContributorInfo, ContributorsReport, OverlapPair, RepoSummary, analyze_contributors,
};
