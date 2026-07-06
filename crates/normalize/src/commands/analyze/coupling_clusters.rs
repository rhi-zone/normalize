//! Change coupling clusters — index-load orchestration.
//!
//! The union-find clustering compute and the `OutputFormatter` presentation live
//! in `normalize_git_history::coupling_clusters` (presentation gated behind that
//! crate's `cli` feature). This module keeps the main-crate-only orchestration:
//! loading co-change edges from the structural index (daemon wiring) with a
//! git-history-walk fallback, then delegating to `cluster_from_edges`.

use std::path::Path;

pub use normalize_git_history::coupling_clusters::{CouplingClustersReport, FileCluster};

/// Analyze temporal coupling clusters.
///
/// Queries the `co_change_edges` table from the structural index when available
/// (populated by `normalize structure rebuild`). Falls back to walking git history
/// directly when the table is empty, with a warning suggesting `structure rebuild`.
pub fn analyze_coupling_clusters(
    root: &Path,
    min_commits: usize,
    limit: usize,
    exclude_patterns: &[String],
    only_patterns: &[String],
) -> Result<CouplingClustersReport, String> {
    // Try to load edges from the index first.
    // `coupling_clusters` is a sync service method running inside a tokio runtime,
    // so we use block_in_place to safely run async code from this sync context.
    let index_edges: Option<Vec<(String, String, usize)>> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(try_load_edges_from_index(root, min_commits))
    });

    // Build coupling pairs: either from the index or by walking git history.
    let raw_pairs: Vec<(String, String, usize)> = if let Some(edges) = index_edges {
        edges
    } else {
        tracing::warn!(
            "co_change_edges table is empty — falling back to git history walk. \
             Run `normalize structure rebuild` to pre-compute the co-change index."
        );
        // Fall back: walk git history via existing coupling analysis.
        let coupling = normalize_git_history::coupling::analyze_coupling(
            root,
            min_commits,
            usize::MAX,
            exclude_patterns,
        )?;
        coupling
            .pairs
            .iter()
            .map(|p| (p.file_a.clone(), p.file_b.clone(), p.shared_commits))
            .collect()
    };

    Ok(
        normalize_git_history::coupling_clusters::cluster_from_edges(
            raw_pairs,
            limit,
            exclude_patterns,
            only_patterns,
        ),
    )
}

/// Try to load co-change edges from the structural index.
///
/// Returns `Some(edges)` when the index has co-change data, or `None` when the table
/// is empty (index not yet built or the project has no git history).
async fn try_load_edges_from_index(
    root: &Path,
    min_commits: usize,
) -> Option<Vec<(String, String, usize)>> {
    let idx = crate::index::ensure_ready_or_warn(root).await?;
    match idx.query_co_change_edges(min_commits).await {
        Ok(edges) => edges,
        Err(e) => {
            tracing::debug!("failed to query co_change_edges: {}", e);
            None
        }
    }
}
