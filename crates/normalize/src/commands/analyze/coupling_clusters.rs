//! Change coupling clusters — group files into connected components of temporal coupling.

use crate::commands::analyze::clusters::UnionFind;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::Path;

/// A cluster of files that change together.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileCluster {
    pub files: Vec<String>,
    /// Number of co-change pairs within the cluster
    pub internal_edges: usize,
    /// Sum of shared commits across all internal edges
    pub total_shared_commits: usize,
    /// internal_edges / max_possible_edges (n*(n-1)/2)
    pub cohesion: f64,
}

/// Report from change coupling cluster analysis.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CouplingClustersReport {
    pub clusters: Vec<FileCluster>,
    pub total_files: usize,
    pub total_clusters: usize,
    pub unclustered_files: usize,
}

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
        let coupling =
            super::coupling::analyze_coupling(root, min_commits, usize::MAX, exclude_patterns)?;
        coupling
            .pairs
            .iter()
            .map(|p| (p.file_a.clone(), p.file_b.clone(), p.shared_commits))
            .collect()
    };

    // When coming from the index, we need to apply exclude patterns ourselves
    // (the git-walk path filters via analyze_coupling, but the index path does not).
    let excludes: Vec<glob::Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let raw_pairs: Vec<(String, String, usize)> = raw_pairs
        .into_iter()
        .filter(|(a, b, _)| {
            !excludes.iter().any(|pat| pat.matches(a)) && !excludes.iter().any(|pat| pat.matches(b))
        })
        .collect();

    // Wrap into a CouplingReport-compatible structure so we can reuse the cluster logic.
    // We don't need commits_a/commits_b for clustering; shared_commits is sufficient.
    let coupling_pairs: Vec<super::coupling::CoupledPair> = raw_pairs
        .iter()
        .map(|(a, b, shared)| super::coupling::CoupledPair {
            file_a: a.clone(),
            file_b: b.clone(),
            shared_commits: *shared,
            commits_a: 0,
            commits_b: 0,
            confidence: 0.0,
            pair_key: format!("{}::{}", a, b),
            delta: None,
        })
        .collect();

    let coupling = super::coupling::CouplingReport {
        pairs: coupling_pairs,
        repos: None,
        diff_ref: None,
    };

    let only_globs: Vec<glob::Pattern> = only_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    // Collect unique files and build index
    let mut file_set = std::collections::HashMap::new();
    let mut files = Vec::new();

    let add_file = |f: &str,
                    file_set: &mut std::collections::HashMap<String, usize>,
                    files: &mut Vec<String>|
     -> Option<usize> {
        if !only_globs.is_empty() && !only_globs.iter().any(|pat| pat.matches(f)) {
            return None;
        }
        let len = file_set.len();
        let idx = *file_set.entry(f.to_string()).or_insert_with(|| {
            files.push(f.to_string());
            len
        });
        Some(idx)
    };

    // Build edges from coupling pairs, filtering by --only
    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    for pair in &coupling.pairs {
        let a = add_file(&pair.file_a, &mut file_set, &mut files);
        let b = add_file(&pair.file_b, &mut file_set, &mut files);
        if let (Some(ai), Some(bi)) = (a, b) {
            edges.push((ai, bi, pair.shared_commits));
        }
    }

    let total_files = files.len();

    // Union-find on edges
    let mut uf = UnionFind::new(total_files);
    for &(a, b, _) in &edges {
        uf.union(a, b);
    }

    // Collect components
    let mut components: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for i in 0..total_files {
        let root = uf.find(i);
        components.entry(root).or_default().push(i);
    }

    // Build clusters (only 2+ files)
    let mut clusters: Vec<FileCluster> = Vec::new();
    let mut unclustered = 0;

    for members in components.values() {
        if members.len() < 2 {
            unclustered += 1;
            continue;
        }
        let member_set: std::collections::HashSet<usize> = members.iter().copied().collect();

        let mut internal_edges = 0;
        let mut total_shared = 0;
        for &(a, b, shared) in &edges {
            if member_set.contains(&a) && member_set.contains(&b) {
                internal_edges += 1;
                total_shared += shared;
            }
        }

        let n = members.len();
        let max_edges = n * (n - 1) / 2;
        let cohesion = if max_edges > 0 {
            internal_edges as f64 / max_edges as f64
        } else {
            0.0
        };

        let mut cluster_files: Vec<String> = members.iter().map(|&i| files[i].clone()).collect();
        cluster_files.sort();

        clusters.push(FileCluster {
            files: cluster_files,
            internal_edges,
            total_shared_commits: total_shared,
            cohesion,
        });
    }

    // Sort by total shared commits descending
    normalize_analyze::ranked::rank_and_truncate(
        &mut clusters,
        limit,
        |a, b| {
            b.total_shared_commits
                .cmp(&a.total_shared_commits)
                .then_with(|| b.files.len().cmp(&a.files.len()))
        },
        |c| c.total_shared_commits as f64,
    );

    let total_clusters = clusters.len();

    Ok(CouplingClustersReport {
        clusters,
        total_files,
        total_clusters,
        unclustered_files: unclustered,
    })
}

impl OutputFormatter for CouplingClustersReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("# Change Clusters".to_string());
        lines.push(format!(
            "{} clusters from {} files ({} unclustered)",
            self.total_clusters, self.total_files, self.unclustered_files
        ));
        lines.push(String::new());

        for (i, cluster) in self.clusters.iter().enumerate() {
            lines.push(format!(
                "{}. {} files  {} edges  cohesion {:.0}%  ({} shared commits)",
                i + 1,
                cluster.files.len(),
                cluster.internal_edges,
                cluster.cohesion * 100.0,
                cluster.total_shared_commits,
            ));
            for f in &cluster.files {
                lines.push(format!("   {}", f));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push("\x1b[1m# Change Clusters\x1b[0m".to_string());
        lines.push(format!(
            "{} clusters from {} files ({} unclustered)",
            self.total_clusters, self.total_files, self.unclustered_files
        ));
        lines.push(String::new());

        for (i, cluster) in self.clusters.iter().enumerate() {
            let cohesion_pct = cluster.cohesion * 100.0;
            let cohesion_color = if cohesion_pct >= 80.0 {
                "\x1b[32m" // green
            } else if cohesion_pct >= 50.0 {
                "\x1b[33m" // yellow
            } else {
                "\x1b[31m" // red
            };
            lines.push(format!(
                "\x1b[1;36m{}.\x1b[0m {} files  {} edges  {}cohesion {:.0}%\x1b[0m  ({} shared commits)",
                i + 1,
                cluster.files.len(),
                cluster.internal_edges,
                cohesion_color,
                cohesion_pct,
                cluster.total_shared_commits,
            ));
            for f in &cluster.files {
                lines.push(format!("   {}", f));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
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
