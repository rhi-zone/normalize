//! Git blame ownership analysis — re-export shim.
//!
//! Typed compute and presentation both live in `normalize_git_history::ownership`
//! (the `OutputFormatter` impl is gated behind that crate's `cli` feature).

pub use normalize_git_history::ownership::{
    FileOwnership, OwnershipRepoEntry, OwnershipReport, analyze_ownership,
};
