//! Structural clustering: group similar functions into connected-component families.
//!
//! Builds on `similar-functions` pairs via union-find to identify which functions
//! form a "family" — functions that are mutually similar, possibly across many files.
//! Ranked by total line count (largest families first).

use crate::commands::analyze::duplicates::find_similar_function_pairs;
use crate::filter::Filter;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// A single function in a cluster.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FunctionNode {
    pub file: String,
    pub symbol: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl FunctionNode {
    fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// A connected component of mutually-similar functions.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FunctionCluster {
    /// All functions in this cluster.
    pub members: Vec<FunctionNode>,
    /// Total lines across all members.
    pub total_lines: usize,
    /// Average pairwise similarity within the cluster.
    pub avg_similarity: f64,
    /// Number of similar-function pairs within this cluster.
    pub pair_count: usize,
}

/// Report from structural clustering analysis.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ClustersReport {
    pub files_scanned: usize,
    pub functions_analyzed: usize,
    pub pairs_analyzed: usize,
    pub cluster_count: usize,
    pub total_clustered_functions: usize,
    pub clusters: Vec<FunctionCluster>,
}

impl OutputFormatter for ClustersReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Structural Clusters".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned:       {}", self.files_scanned));
        lines.push(format!("Functions analyzed:  {}", self.functions_analyzed));
        lines.push(format!("Pairs analyzed:      {}", self.pairs_analyzed));
        lines.push(format!(
            "Clusters found:      {}  ({} functions)",
            self.cluster_count, self.total_clustered_functions
        ));

        if self.clusters.is_empty() {
            lines.push(String::new());
            lines.push("No function clusters detected.".to_string());
            return lines.join("\n");
        }

        lines.push(String::new());

        for (i, cluster) in self.clusters.iter().enumerate() {
            lines.push(format!(
                "{}. {} functions  {} lines  avg {:.0}% similar  ({} pairs)",
                i + 1,
                cluster.members.len(),
                cluster.total_lines,
                cluster.avg_similarity * 100.0,
                cluster.pair_count,
            ));
            for member in &cluster.members {
                lines.push(format!(
                    "   {}:{}  (lines {}-{})",
                    member.file, member.symbol, member.start_line, member.end_line,
                ));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

/// Union-Find for grouping functions into connected components.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

pub struct ClustersConfig<'a> {
    pub roots: &'a [PathBuf],
    pub min_lines: usize,
    pub similarity: f64,
    pub elide_identifiers: bool,
    pub skeleton: bool,
    pub include_trait_impls: bool,
    pub limit: usize,
    pub filter: Option<&'a Filter>,
}

/// Build a clusters report from the given root (single-repo convenience wrapper).
pub fn build_clusters_report(
    root: &std::path::Path,
    min_lines: usize,
    similarity: f64,
    skeleton: bool,
    include_trait_impls: bool,
    limit: usize,
    filter: Option<&Filter>,
) -> ClustersReport {
    let roots_vec = vec![root.to_path_buf()];
    build_clusters_report_multi(
        &roots_vec,
        min_lines,
        similarity,
        true, // elide_identifiers
        skeleton,
        include_trait_impls,
        limit,
        filter,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_clusters_report_multi(
    roots: &[PathBuf],
    min_lines: usize,
    similarity: f64,
    elide_identifiers: bool,
    skeleton: bool,
    include_trait_impls: bool,
    limit: usize,
    filter: Option<&Filter>,
) -> ClustersReport {
    let result = find_similar_function_pairs(
        roots,
        min_lines,
        similarity,
        elide_identifiers,
        false, // elide_literals
        skeleton,
        include_trait_impls,
        filter,
    );
    let files_scanned = result.files_scanned;
    let functions_analyzed = result.functions_analyzed;
    let pairs = result.pairs;

    let pairs_analyzed = pairs.len();

    // Build function index: unique key -> id
    type FnKey = (String, String, usize, usize);
    let mut fn_map: HashMap<FnKey, usize> = HashMap::new();
    let mut fn_list: Vec<FunctionNode> = Vec::new();

    for pair in &pairs {
        for (file, sym, start, end) in [
            (
                &pair.file_a,
                &pair.symbol_a,
                pair.start_line_a,
                pair.end_line_a,
            ),
            (
                &pair.file_b,
                &pair.symbol_b,
                pair.start_line_b,
                pair.end_line_b,
            ),
        ] {
            let key = (file.clone(), sym.clone(), start, end);
            if let std::collections::hash_map::Entry::Vacant(e) = fn_map.entry(key) {
                e.insert(fn_list.len());
                fn_list.push(FunctionNode {
                    file: file.clone(),
                    symbol: sym.clone(),
                    start_line: start,
                    end_line: end,
                });
            }
        }
    }

    let n = fn_list.len();
    let mut uf = UnionFind::new(n);

    // Track per-pair (id_a, id_b, similarity) for computing cluster avg similarity
    let mut pair_data: Vec<(usize, usize, f64)> = Vec::with_capacity(pairs.len());

    for pair in &pairs {
        let id_a = fn_map[&(
            pair.file_a.clone(),
            pair.symbol_a.clone(),
            pair.start_line_a,
            pair.end_line_a,
        )];
        let id_b = fn_map[&(
            pair.file_b.clone(),
            pair.symbol_b.clone(),
            pair.start_line_b,
            pair.end_line_b,
        )];
        uf.union(id_a, id_b);
        pair_data.push((id_a, id_b, pair.similarity));
    }

    // Collect connected components (root -> member ids)
    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        components.entry(root).or_default().push(i);
    }

    // Build clusters from components with 2+ members
    let mut clusters: Vec<FunctionCluster> = components
        .into_values()
        .filter(|members| members.len() >= 2)
        .map(|member_ids| {
            let member_set: std::collections::HashSet<usize> = member_ids.iter().copied().collect();

            let members: Vec<FunctionNode> =
                member_ids.iter().map(|&id| fn_list[id].clone()).collect();

            let total_lines: usize = members.iter().map(|f| f.line_count()).sum();

            // Average similarity of pairs within this cluster
            let cluster_pairs: Vec<f64> = pair_data
                .iter()
                .filter(|(a, b, _)| member_set.contains(a) && member_set.contains(b))
                .map(|(_, _, s)| *s)
                .collect();

            let avg_similarity = if cluster_pairs.is_empty() {
                0.0
            } else {
                cluster_pairs.iter().sum::<f64>() / cluster_pairs.len() as f64
            };

            FunctionCluster {
                members,
                total_lines,
                avg_similarity,
                pair_count: cluster_pairs.len(),
            }
        })
        .collect();

    // Sort by total lines descending (largest families first)
    normalize_analyze::ranked::rank_and_truncate(
        &mut clusters,
        limit,
        |a, b| {
            b.total_lines
                .cmp(&a.total_lines)
                .then_with(|| b.members.len().cmp(&a.members.len()))
        },
        |c| c.total_lines as f64,
    );

    let cluster_count = clusters.len();
    let total_clustered_functions: usize = clusters.iter().map(|c| c.members.len()).sum();

    ClustersReport {
        files_scanned,
        functions_analyzed,
        pairs_analyzed,
        cluster_count,
        total_clustered_functions,
        clusters,
    }
}

pub fn cmd_clusters(cfg: ClustersConfig<'_>) -> i32 {
    let ClustersConfig {
        roots,
        min_lines,
        similarity,
        elide_identifiers,
        skeleton,
        include_trait_impls,
        limit,
        filter,
    } = cfg;

    let report = build_clusters_report_multi(
        roots,
        min_lines,
        similarity,
        elide_identifiers,
        skeleton,
        include_trait_impls,
        limit,
        filter,
    );

    println!("{}", report.format_text());
    0
}
