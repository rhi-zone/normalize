//! Cross-repo activity over time — re-export shim.
//!
//! Typed compute and presentation both live in `normalize_git_history::activity`
//! (the `OutputFormatter` impl is gated behind that crate's `cli` feature).

pub use normalize_git_history::activity::{
    ActivityReport, RepoActivity, Trend, WindowActivity, WindowGranularity, analyze_activity,
};
