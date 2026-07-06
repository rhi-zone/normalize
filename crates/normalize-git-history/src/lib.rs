//! Typed code-health analysis derived from git history.
//!
//! This crate holds the compute layer for the git-history analysis family:
//! churn hotspots, temporal coupling, blame ownership, cross-repo contributors,
//! activity-over-time, cross-repo coupling, and change-coupling clusters. Each
//! module exposes typed report structs plus a pure `analyze_*` (or
//! `cluster_from_edges`) entry point.
//!
//! Presentation (text/pretty rendering, `OutputFormatter`) lives in the
//! `normalize` binary crate — this crate is renderer-agnostic and publishable
//! standalone. It will back the `normalize history` verb.

mod complexity;

pub mod activity;
pub mod contributors;
pub mod coupling;
pub mod coupling_clusters;
pub mod hotspots;
pub mod ownership;
pub mod repo_coupling;

/// The server-less `HistoryService` (`history` verb). Behind `cli` because it
/// pulls the CLI/rendering/index stack; the compute API needs none of it.
#[cfg(feature = "cli")]
pub mod service;

pub use complexity::max_function_complexity;

use std::path::Path;

/// Check if a path is a source file we can analyze.
pub fn is_source_file(path: &Path) -> bool {
    !is_generated_file(path) && normalize_languages::support_for_path(path).is_some()
}

/// Known generated/lockfiles that are not useful to analyze for code quality.
fn is_generated_file(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // Common lock files by exact name
    matches!(
        file_name,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lockb"
            | "Cargo.lock"
            | "composer.lock"
            | "Gemfile.lock"
            | "poetry.lock"
            | "Pipfile.lock"
            | "packages.lock.json"
    ) || file_name.ends_with(".lock")
}
