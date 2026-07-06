//! Change coupling clusters — group files into connected components of temporal coupling.

use normalize_code_similarity::UnionFind;
use serde::Serialize;

#[cfg(feature = "cli")]
mod present {
    use super::CouplingClustersReport;
    use normalize_output::OutputFormatter;

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
}

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

/// Cluster co-change edges into connected components via union-find.
///
/// `raw_pairs` are `(file_a, file_b, shared_commits)` co-change edges (from the
/// structural index or a git-history walk). `exclude_patterns` drop any pair
/// touching a matching path; `only_patterns` restrict clustering to matching
/// files. Clusters (2+ files) are ranked by total shared commits and truncated
/// to `limit`.
pub fn cluster_from_edges(
    raw_pairs: Vec<(String, String, usize)>,
    limit: usize,
    exclude_patterns: &[String],
    only_patterns: &[String],
) -> CouplingClustersReport {
    // Apply exclude patterns (the git-walk path filters via analyze_coupling, but
    // the index path does not — filter uniformly here).
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
    for (file_a, file_b, shared_commits) in &raw_pairs {
        let a = add_file(file_a, &mut file_set, &mut files);
        let b = add_file(file_b, &mut file_set, &mut files);
        if let (Some(ai), Some(bi)) = (a, b) {
            edges.push((ai, bi, *shared_commits));
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
    normalize_rank::ranked::rank_and_truncate(
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

    CouplingClustersReport {
        clusters,
        total_files,
        total_clusters,
        unclustered_files: unclustered,
    }
}
