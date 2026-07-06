//! Git history hotspot analysis — re-export shim.
//!
//! Typed compute and presentation both live in `normalize_git_history::hotspots`
//! (the `OutputFormatter` impl is gated behind that crate's `cli` feature, which
//! the `normalize` binary enables). This module re-exports the API so existing
//! `crate::commands::analyze::hotspots::…` paths keep resolving.

pub use normalize_git_history::hotspots::{
    FileHotspot, HotspotsRepoEntry, HotspotsReport, analyze_hotspots,
};
