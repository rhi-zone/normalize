//! Git utility functions — re-exported from `normalize_git`.
//!
//! All implementation lives in the `normalize-git` crate; this module is a
//! thin re-export so existing call sites (`git_utils::open_repo`, etc.) continue
//! to compile without change.
pub use normalize_git::*;
