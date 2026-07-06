//! Temporal coupling analysis — re-export shim.
//!
//! Typed compute and presentation both live in `normalize_git_history::coupling`
//! (the `OutputFormatter` impl is gated behind that crate's `cli` feature).

pub use normalize_git_history::coupling::{
    CoupledPair, CouplingRepoEntry, CouplingReport, analyze_coupling,
};
