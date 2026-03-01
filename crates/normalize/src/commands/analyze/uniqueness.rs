use normalize_languages::support_for_path;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::commands::analyze::duplicates::find_similar_function_pairs;
use crate::commands::analyze::test_ratio::module_key;
use crate::output::OutputFormatter;

/// Per-module uniqueness breakdown.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleUniqueness {
    pub module: String,
    pub total_functions: usize,
    /// Functions with no structural near-twin found.
    pub unique_functions: usize,
    /// Functions that share a structural cluster with at least one other.
    pub clustered_functions: usize,
    /// unique / total
    pub uniqueness_ratio: f64,
    pub total_lines: usize,
    pub clustered_lines: usize,
}

/// Summary of a structural cluster (group of near-duplicate functions).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ClusterSummary {
    /// Number of functions in the cluster.
    pub size: usize,
    pub total_lines: usize,
    /// Representative "file:symbol" from first member.
    pub representative: String,
    /// How many distinct modules this cluster spans.
    pub modules_spanned: usize,
}

/// Report returned by `analyze uniqueness`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct UniquenessReport {
    pub root: String,
    pub files_analyzed: usize,
    pub total_functions: usize,
    pub unique_functions: usize,
    pub clustered_functions: usize,
    pub overall_uniqueness_ratio: f64,
    pub similarity_threshold: f64,
    /// Modules sorted by uniqueness_ratio ascending (most clustered first).
    pub modules: Vec<ModuleUniqueness>,
    /// Largest structural clusters.
    pub top_clusters: Vec<ClusterSummary>,
}

impl OutputFormatter for UniquenessReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();
        out.push("# Function Uniqueness Analysis".to_string());
        out.push(String::new());
        out.push(format!("Root:                 {}", self.root));
        out.push(format!("Files analyzed:       {}", self.files_analyzed));
        out.push(format!("Functions analyzed:   {}", self.total_functions));
        out.push(format!(
            "Unique functions:     {}  ({:.1}%)",
            self.unique_functions,
            self.overall_uniqueness_ratio * 100.0
        ));
        out.push(format!(
            "Clustered functions:  {}  ({:.1}%)",
            self.clustered_functions,
            if self.total_functions > 0 {
                self.clustered_functions as f64 / self.total_functions as f64 * 100.0
            } else {
                0.0
            }
        ));
        out.push(format!(
            "Similarity threshold: {:.0}%",
            self.similarity_threshold * 100.0
        ));
        out.push(String::new());

        if !self.modules.is_empty() {
            out.push("## Modules (most clustered first)".to_string());
            out.push(String::new());
            let w = self
                .modules
                .iter()
                .map(|m| m.module.len())
                .max()
                .unwrap_or(20);
            out.push(format!(
                "  {:<w$}  {:>5}  {:>9}  {:>9}  {:>8}",
                "module",
                "fns",
                "unique",
                "clustered",
                "ratio",
                w = w
            ));
            out.push(format!(
                "  {:<w$}  {:>5}  {:>9}  {:>9}  {:>8}",
                "-".repeat(w),
                "-----",
                "---------",
                "---------",
                "--------",
                w = w
            ));
            for m in &self.modules {
                out.push(format!(
                    "  {:<w$}  {:>5}  {:>9}  {:>9}  {:>7.1}%",
                    m.module,
                    m.total_functions,
                    m.unique_functions,
                    m.clustered_functions,
                    m.uniqueness_ratio * 100.0,
                    w = w
                ));
            }
        }

        if !self.top_clusters.is_empty() {
            out.push(String::new());
            out.push("## Top Structural Clusters".to_string());
            out.push(String::new());
            for (i, c) in self.top_clusters.iter().enumerate() {
                let cross = if c.modules_spanned > 1 {
                    format!(
                        "  [spans {} modules — abstraction candidate]",
                        c.modules_spanned
                    )
                } else {
                    String::new()
                };
                out.push(format!(
                    "{}. {} functions  {} lines  representative: {}{}",
                    i + 1,
                    c.size,
                    c.total_lines,
                    c.representative,
                    cross
                ));
            }
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;
        let mut out = Vec::new();
        out.push(
            Color::Cyan
                .bold()
                .paint("# Function Uniqueness Analysis")
                .to_string(),
        );
        out.push(String::new());
        out.push(format!("Root:                 {}", self.root));
        out.push(format!("Files analyzed:       {}", self.files_analyzed));
        out.push(format!("Functions analyzed:   {}", self.total_functions));
        out.push(format!(
            "Unique functions:     {}  ({:.1}%)",
            self.unique_functions,
            self.overall_uniqueness_ratio * 100.0
        ));
        out.push(format!(
            "Clustered functions:  {}  ({:.1}%)",
            self.clustered_functions,
            if self.total_functions > 0 {
                self.clustered_functions as f64 / self.total_functions as f64 * 100.0
            } else {
                0.0
            }
        ));
        out.push(format!(
            "Similarity threshold: {:.0}%",
            self.similarity_threshold * 100.0
        ));
        out.push(String::new());

        if !self.modules.is_empty() {
            out.push(
                Color::Yellow
                    .bold()
                    .paint("## Modules (most clustered first)")
                    .to_string(),
            );
            out.push(String::new());
            let w = self
                .modules
                .iter()
                .map(|m| m.module.len())
                .max()
                .unwrap_or(20);
            out.push(format!(
                "  {:<w$}  {:>5}  {:>9}  {:>9}  {:>8}",
                Color::White.bold().paint("module"),
                Color::White.bold().paint("fns"),
                Color::White.bold().paint("unique"),
                Color::White.bold().paint("clustered"),
                Color::White.bold().paint("ratio"),
                w = w
            ));
            for m in &self.modules {
                let color = if m.uniqueness_ratio < 0.5 {
                    Color::Red
                } else if m.uniqueness_ratio < 0.75 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                out.push(format!(
                    "  {:<w$}  {:>5}  {:>9}  {:>9}  {}",
                    m.module,
                    m.total_functions,
                    m.unique_functions,
                    m.clustered_functions,
                    color.paint(format!("{:.1}%", m.uniqueness_ratio * 100.0)),
                    w = w
                ));
            }
        }

        if !self.top_clusters.is_empty() {
            out.push(String::new());
            out.push(
                Color::Yellow
                    .bold()
                    .paint("## Top Structural Clusters")
                    .to_string(),
            );
            out.push(String::new());
            for (i, c) in self.top_clusters.iter().enumerate() {
                let cross = if c.modules_spanned > 1 {
                    Color::Magenta
                        .paint(format!(
                            "  [spans {} modules — abstraction candidate]",
                            c.modules_spanned
                        ))
                        .to_string()
                } else {
                    String::new()
                };
                out.push(format!(
                    "{}. {} functions  {} lines  representative: {}{}",
                    i + 1,
                    c.size,
                    c.total_lines,
                    c.representative,
                    cross
                ));
            }
        }

        out.join("\n")
    }
}

/// Analyze structural uniqueness of functions across the codebase.
#[allow(clippy::too_many_arguments)]
pub fn analyze_uniqueness(
    root: &Path,
    similarity: f64,
    min_lines: usize,
    skeleton: bool,
    include_trait_impls: bool,
    module_limit: usize,
    cluster_limit: usize,
    filter: Option<&crate::filter::Filter>,
) -> UniquenessReport {
    let roots = vec![root.to_path_buf()];

    // Find similar function pairs
    let (files_scanned, _functions_analyzed, pairs) = find_similar_function_pairs(
        &roots,
        min_lines,
        similarity,
        false, // elide_identifiers
        false, // elide_literals
        skeleton,
        include_trait_impls,
        filter,
    );

    // Build set of functions that have at least one twin
    // key: (file, start_line, end_line)
    let mut has_twin: HashSet<(String, usize, usize)> = HashSet::new();
    // Also track cluster membership for cluster summary
    // Use union-find via pair graph
    type FnId = (String, usize, usize); // (file, start_line, end_line)
    let mut all_clustered_fns: Vec<FnId> = Vec::new();

    // Group into clusters via adjacency (simple flood-fill)
    // Build adjacency list
    let mut adj: HashMap<FnId, Vec<FnId>> = HashMap::new();
    for pair in &pairs {
        let a: FnId = (pair.file_a.clone(), pair.start_line_a, pair.end_line_a);
        let b: FnId = (pair.file_b.clone(), pair.start_line_b, pair.end_line_b);
        has_twin.insert(a.clone());
        has_twin.insert(b.clone());
        adj.entry(a.clone()).or_default().push(b.clone());
        adj.entry(b.clone()).or_default().push(a.clone());
    }

    // BFS to find connected components (clusters)
    let mut visited: HashSet<FnId> = HashSet::new();
    let mut clusters: Vec<Vec<FnId>> = Vec::new();
    for start in adj.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut cluster = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start.clone());
        visited.insert(start.clone());
        while let Some(node) = queue.pop_front() {
            cluster.push(node.clone());
            if let Some(neighbors) = adj.get(&node) {
                for nb in neighbors {
                    if visited.insert(nb.clone()) {
                        queue.push_back(nb.clone());
                    }
                }
            }
        }
        all_clustered_fns.extend(cluster.iter().cloned());
        clusters.push(cluster);
    }

    // Now count all functions per module via file walk
    let all_files = crate::path_resolve::all_files(root);

    // (module_key -> (total_fns, lines))
    // Also track per-file function counts for uniqueness
    let file_fn_counts: Vec<(String, usize, usize)> = all_files
        .par_iter()
        .filter(|f| f.kind == "file")
        .filter_map(|f| {
            let abs_path = root.join(&f.path);
            support_for_path(&abs_path)?;
            let content = std::fs::read_to_string(&abs_path).ok()?;
            if content.is_empty() {
                return None;
            }
            let fn_count = count_fns_via_language(&abs_path, &content);
            if fn_count == 0 {
                return None;
            }
            let lines = content.lines().count();
            Some((f.path.clone(), fn_count, lines))
        })
        .collect();

    // Aggregate per module
    let mut module_totals: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (total_fns, total_lines)
    for (path, fn_count, lines) in &file_fn_counts {
        let key = module_key(path);
        let entry = module_totals.entry(key).or_default();
        entry.0 += fn_count;
        entry.1 += lines;
    }

    // Count clustered functions per module
    let mut module_clustered: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (clustered_fns, clustered_lines)
    for (file, start, end) in &has_twin {
        let key = module_key(file);
        let entry = module_clustered.entry(key).or_default();
        entry.0 += 1;
        entry.1 += end.saturating_sub(*start) + 1;
    }

    let total_functions: usize = file_fn_counts.iter().map(|(_, c, _)| c).sum();
    let clustered_functions = has_twin.len();
    let unique_functions = total_functions.saturating_sub(clustered_functions);
    let overall_uniqueness_ratio = if total_functions > 0 {
        unique_functions as f64 / total_functions as f64
    } else {
        1.0
    };

    // Build module entries
    let mut modules: Vec<ModuleUniqueness> = module_totals
        .into_iter()
        .filter(|(_, (total, _))| *total > 0)
        .map(|(module, (total, lines))| {
            let (clustered_fns, clustered_lines) =
                module_clustered.get(&module).copied().unwrap_or((0, 0));
            let unique_fns = total.saturating_sub(clustered_fns);
            let ratio = if total > 0 {
                unique_fns as f64 / total as f64
            } else {
                1.0
            };
            ModuleUniqueness {
                module,
                total_functions: total,
                unique_functions: unique_fns,
                clustered_functions: clustered_fns,
                uniqueness_ratio: ratio,
                total_lines: lines,
                clustered_lines,
            }
        })
        .collect();

    // Sort by uniqueness_ratio ascending (most clustered first), then by size desc
    modules.sort_by(|a, b| {
        a.uniqueness_ratio
            .partial_cmp(&b.uniqueness_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.total_functions.cmp(&a.total_functions))
    });
    if module_limit > 0 {
        modules.truncate(module_limit);
    }

    // Build top cluster summaries
    // Sort clusters by size desc
    let mut cluster_summaries: Vec<ClusterSummary> = clusters
        .into_iter()
        .filter(|c| c.len() >= 2)
        .map(|members| {
            let size = members.len();
            let total_lines: usize = members
                .iter()
                .map(|(_, s, e)| e.saturating_sub(*s) + 1)
                .sum();
            let representative = members
                .first()
                .map(|(file, start, end)| format!("{}:{}-{}", file, start, end))
                .unwrap_or_default();
            let modules_spanned: HashSet<String> =
                members.iter().map(|(f, _, _)| module_key(f)).collect();
            ClusterSummary {
                size,
                total_lines,
                representative,
                modules_spanned: modules_spanned.len(),
            }
        })
        .collect();
    cluster_summaries.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| b.total_lines.cmp(&a.total_lines))
    });
    cluster_summaries.truncate(cluster_limit);

    UniquenessReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        files_analyzed: files_scanned,
        total_functions,
        unique_functions,
        clustered_functions,
        overall_uniqueness_ratio,
        similarity_threshold: similarity,
        modules,
        top_clusters: cluster_summaries,
    }
}

/// Count functions and methods in a file using the language trait's symbol extraction.
fn count_fns_via_language(abs_path: &Path, content: &str) -> usize {
    let support = match support_for_path(abs_path) {
        Some(s) => s,
        None => return 0,
    };
    let grammar = support.grammar_name();
    let tree = match crate::parsers::parse_with_grammar(grammar, content) {
        Some(t) => t,
        None => return 0,
    };
    count_fn_nodes(tree.root_node(), support.function_kinds())
}

fn count_fn_nodes(node: tree_sitter::Node, fn_kinds: &[&str]) -> usize {
    let mut count = 0;
    if fn_kinds.contains(&node.kind()) {
        count += 1;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            count += count_fn_nodes(child, fn_kinds);
        }
    }
    count
}
