//! Fragment frequency analysis — find repeated AST subtrees across the codebase.
//!
//! Extracts AST subtrees ≥ `min_nodes` wide, elides identifiers/literals, hashes,
//! and groups by frequency.  With `--inline-depth N` call nodes are resolved via the
//! index and callee bodies are spliced in before hashing (descending MinHash).
//!
//! Exact mode (`similarity == 1.0`) groups by hash; fuzzy mode uses MinHash + LSH +
//! union-find clustering.

use crate::extract::Extractor;
use crate::output::OutputFormatter;
use normalize_code_similarity::{
    LSH_BANDS, MINHASH_N, UnionFind, compute_minhash, find_function_node, flatten_symbols,
    jaccard_estimate, lsh_band_hash, serialize_subtree_tokens,
};
use normalize_languages::{parsers, support_for_path};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

// ── Data structures ───────────────────────────────────────────────────────────

/// Scope of AST subtrees to consider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentScope {
    /// Any subtree ≥ min_nodes (default)
    All,
    /// Only function/method bodies
    Functions,
    /// Only block-level subtrees (if/loop/match arms)
    Blocks,
}

impl std::str::FromStr for FragmentScope {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "functions" => Ok(Self::Functions),
            "blocks" => Ok(Self::Blocks),
            _ => Err(format!(
                "unknown scope '{}': expected all|functions|blocks",
                s
            )),
        }
    }
}

/// A single occurrence of a fragment in the codebase.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FragmentLocation {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Enclosing function/method name, if any.
    pub symbol: Option<String>,
}

/// A cluster of structurally identical (or similar) AST fragments.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FragmentCluster {
    /// Hex hash representative
    pub hash: String,
    /// Number of occurrences
    pub frequency: usize,
    /// frequency × avg_lines (impact metric)
    pub total_lines: usize,
    pub avg_lines: f64,
    /// Dominant AST node kind (e.g. "if_expression")
    pub node_kind: String,
    /// Structural summary: [("if", 2), ("call", 3)]
    pub label: Vec<(String, usize)>,
    /// Average pairwise similarity within the cluster (fuzzy mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_similarity: Option<f64>,
    pub members: Vec<FragmentLocation>,
}

/// Full fragments analysis report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FragmentsReport {
    pub clusters: Vec<FragmentCluster>,
    pub total_fragments_scanned: usize,
    pub total_clusters: usize,
    /// Fragments not assigned to any cluster (below min_members threshold).
    pub unclustered_count: usize,
    pub inline_depth: usize,
    pub min_nodes: usize,
    pub stats: normalize_analyze::ranked::RankStats,
}

// ── Internal types ────────────────────────────────────────────────────────────

/// A candidate fragment extracted from a single file.
struct Fragment {
    file: String,
    start_line: usize,
    end_line: usize,
    symbol: Option<String>,
    node_kind: String,
    kind_counts: Vec<(String, usize)>,
    tokens: Vec<u64>,
}

// ── Core analysis ─────────────────────────────────────────────────────────────

/// Run fragment frequency analysis on all source files under `root`.
#[allow(clippy::too_many_arguments)]
pub fn analyze_fragments(
    root: &PathBuf,
    min_nodes: usize,
    scope: FragmentScope,
    entry: Option<&str>,
    inline_depth: usize,
    similarity: f64,
    limit: usize,
    skeleton: bool,
    min_members: usize,
    exclude: &[String],
    only: &[String],
) -> Result<FragmentsReport, String> {
    if entry.is_some() && scope != FragmentScope::Functions {
        return Err("--entry requires --scope functions".to_string());
    }

    let extractor = Extractor::new();
    let filter = crate::commands::build_filter(root, exclude, only);

    // Inline depth > 0 requires async index — scaffolded but not yet wired.
    if inline_depth > 0 {
        eprintln!("warning: --inline-depth > 0 is not yet implemented; ignoring");
    }

    let files: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file() && super::is_source_file(path)
        })
        .filter(|e| {
            if let Some(ref f) = filter {
                let rel_path = e.path().strip_prefix(root).unwrap_or(e.path());
                f.matches(rel_path)
            } else {
                true
            }
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let per_file: Vec<Vec<Fragment>> = files
        .par_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            let lang = support_for_path(path)?;
            let tree = parsers::parse_with_grammar(lang.grammar_name(), &content)?;
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string();

            let result = extractor.extract(path, &content);
            let flat_syms: Vec<_> = result
                .symbols
                .iter()
                .flat_map(|s| flatten_symbols(s))
                .collect();

            // Build container map: symbol_name@start_line → container_name
            let container_map = build_container_map(&result.symbols);

            let mut fragments = Vec::new();

            match scope {
                FragmentScope::Functions => {
                    for sym in &flat_syms {
                        let kind = sym.kind.as_str();
                        if kind != "function" && kind != "method" {
                            continue;
                        }
                        if let Some(pat) = entry {
                            let container = container_map
                                .get(&(sym.name.clone(), sym.start_line))
                                .map(|s| s.as_str());
                            if !matches_entry(&rel_path, container, &sym.name, pat) {
                                continue;
                            }
                        }
                        if let Some(node) = find_function_node(&tree, sym.start_line) {
                            let child_count = count_descendants(&node);
                            if child_count < min_nodes {
                                continue;
                            }
                            let mut tokens = Vec::new();
                            serialize_subtree_tokens(
                                &node,
                                content.as_bytes(),
                                true,
                                true,
                                skeleton,
                                &mut tokens,
                            );
                            if tokens.len() < 3 {
                                continue;
                            }
                            let mut kind_counts = HashMap::new();
                            collect_node_kinds(&node, &mut kind_counts);
                            let mut kv: Vec<_> = kind_counts.into_iter().collect();
                            kv.sort_by(|a, b| b.1.cmp(&a.1));

                            fragments.push(Fragment {
                                file: rel_path.clone(),
                                start_line: sym.start_line,
                                end_line: sym.end_line,
                                symbol: Some(sym.name.clone()),
                                node_kind: node.kind().to_string(),
                                kind_counts: kv,
                                tokens,
                            });
                        }
                    }
                }
                FragmentScope::Blocks => {
                    walk_blocks(
                        &tree.root_node(),
                        content.as_bytes(),
                        min_nodes,
                        skeleton,
                        &rel_path,
                        &flat_syms,
                        &mut fragments,
                    );
                }
                FragmentScope::All => {
                    walk_all_subtrees(
                        &tree.root_node(),
                        content.as_bytes(),
                        min_nodes,
                        skeleton,
                        &rel_path,
                        &flat_syms,
                        &mut fragments,
                    );
                }
            }

            if fragments.is_empty() {
                None
            } else {
                Some(fragments)
            }
        })
        .collect();

    let all_fragments: Vec<Fragment> = per_file.into_iter().flatten().collect();
    let total_fragments_scanned = all_fragments.len();

    if total_fragments_scanned == 0 {
        return Ok(FragmentsReport {
            clusters: Vec::new(),
            total_fragments_scanned: 0,
            total_clusters: 0,
            unclustered_count: 0,
            inline_depth,
            min_nodes,
            stats: normalize_analyze::ranked::RankStats::from_scores(std::iter::empty()),
        });
    }

    let effective_min = min_members.max(2);
    let is_fuzzy = similarity < 1.0;

    let FuzzyGroups {
        clusters,
        pair_sims,
    } = if is_fuzzy {
        group_fuzzy(&all_fragments, similarity)
    } else {
        FuzzyGroups {
            clusters: group_exact(&all_fragments),
            pair_sims: HashMap::new(),
        }
    };

    let clustered_count: usize = clusters
        .iter()
        .filter(|idxs| idxs.len() >= effective_min)
        .map(|idxs| idxs.len())
        .sum();

    let mut result_clusters: Vec<FragmentCluster> = clusters
        .into_iter()
        .filter(|idxs| idxs.len() >= effective_min)
        .map(|idxs| build_cluster(&all_fragments, &idxs, is_fuzzy, &pair_sims))
        .collect();

    let stats = normalize_analyze::ranked::rank_and_truncate(
        &mut result_clusters,
        limit,
        |a, b| {
            b.total_lines
                .cmp(&a.total_lines)
                .then(b.frequency.cmp(&a.frequency))
        },
        |c| c.total_lines as f64,
    );

    Ok(FragmentsReport {
        total_clusters: result_clusters.len(),
        clusters: result_clusters,
        total_fragments_scanned,
        unclustered_count: total_fragments_scanned - clustered_count,
        inline_depth,
        min_nodes,
        stats,
    })
}

// ── Fragment extraction helpers ───────────────────────────────────────────────

/// Count total descendants of a node.
fn count_descendants(node: &tree_sitter::Node) -> usize {
    let mut count = 1;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            count += count_descendants(&cursor.node());
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    count
}

/// Collect node kind frequency counts from a subtree.
fn collect_node_kinds(node: &tree_sitter::Node, counts: &mut HashMap<String, usize>) {
    let kind = node.kind();
    if node.child_count() > 0 {
        *counts.entry(kind.to_string()).or_default() += 1;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_node_kinds(&cursor.node(), counts);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Check if a block-level node kind is interesting (if/loop/match/etc.).
fn is_block_kind(kind: &str) -> bool {
    kind.contains("if")
        || kind.contains("match")
        || kind.contains("switch")
        || kind.contains("for")
        || kind.contains("while")
        || kind.contains("loop")
        || kind.contains("case")
        || kind.contains("try")
        || kind.contains("block")
}

/// Walk tree collecting block-level fragments.
fn walk_blocks(
    node: &tree_sitter::Node,
    content: &[u8],
    min_nodes: usize,
    skeleton: bool,
    rel_path: &str,
    syms: &[&normalize_languages::Symbol],
    out: &mut Vec<Fragment>,
) {
    if is_block_kind(node.kind()) && count_descendants(node) >= min_nodes {
        let mut tokens = Vec::new();
        serialize_subtree_tokens(node, content, true, true, skeleton, &mut tokens);
        if tokens.len() >= 3 {
            let enclosing = find_enclosing_symbol(node, syms);
            let mut kind_counts = HashMap::new();
            collect_node_kinds(node, &mut kind_counts);
            let mut kv: Vec<_> = kind_counts.into_iter().collect();
            kv.sort_by(|a, b| b.1.cmp(&a.1));
            out.push(Fragment {
                file: rel_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                symbol: enclosing,
                node_kind: node.kind().to_string(),
                kind_counts: kv,
                tokens,
            });
        }
    }
    // Recurse into children
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_blocks(
                &cursor.node(),
                content,
                min_nodes,
                skeleton,
                rel_path,
                syms,
                out,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Walk tree collecting all subtrees ≥ min_nodes.
fn walk_all_subtrees(
    node: &tree_sitter::Node,
    content: &[u8],
    min_nodes: usize,
    skeleton: bool,
    rel_path: &str,
    syms: &[&normalize_languages::Symbol],
    out: &mut Vec<Fragment>,
) {
    if node.child_count() > 0 && count_descendants(node) >= min_nodes {
        let mut tokens = Vec::new();
        serialize_subtree_tokens(node, content, true, true, skeleton, &mut tokens);
        if tokens.len() >= 3 {
            let enclosing = find_enclosing_symbol(node, syms);
            let mut kind_counts = HashMap::new();
            collect_node_kinds(node, &mut kind_counts);
            let mut kv: Vec<_> = kind_counts.into_iter().collect();
            kv.sort_by(|a, b| b.1.cmp(&a.1));
            out.push(Fragment {
                file: rel_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                symbol: enclosing,
                node_kind: node.kind().to_string(),
                kind_counts: kv,
                tokens,
            });
        }
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all_subtrees(
                &cursor.node(),
                content,
                min_nodes,
                skeleton,
                rel_path,
                syms,
                out,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Build a map from (child_name, start_line) → parent container name.
fn build_container_map(
    symbols: &[normalize_languages::Symbol],
) -> HashMap<(String, usize), String> {
    let mut map = HashMap::new();
    for sym in symbols {
        for child in &sym.children {
            collect_container_entries(child, &sym.name, &mut map);
        }
    }
    map
}

fn collect_container_entries(
    sym: &normalize_languages::Symbol,
    parent: &str,
    map: &mut HashMap<(String, usize), String>,
) {
    map.insert((sym.name.clone(), sym.start_line), parent.to_string());
    for child in &sym.children {
        collect_container_entries(child, &sym.name, map);
    }
}

/// Find the enclosing function/method symbol for a given node.
fn find_enclosing_symbol(
    node: &tree_sitter::Node,
    syms: &[&normalize_languages::Symbol],
) -> Option<String> {
    let line = node.start_position().row + 1;
    syms.iter()
        .filter(|s| {
            let k = s.kind.as_str();
            (k == "function" || k == "method") && s.start_line <= line && s.end_line >= line
        })
        .min_by_key(|s| s.end_line - s.start_line) // tightest enclosing
        .map(|s| s.name.clone())
}

/// Match a symbol against an entry pattern (unified path glob).
///
/// Pattern segments are matched against `file/container/symbol`.
/// A bare pattern (no `/`) matches the symbol name only.
fn matches_entry(file: &str, container: Option<&str>, symbol: &str, pattern: &str) -> bool {
    // Normalize separators (::, #, : → /)
    let norm_pattern = pattern.replace("::", "/").replace(['#', ':'], "/");

    if !norm_pattern.contains('/') {
        // Bare pattern: match symbol name only
        return glob_match(&norm_pattern, symbol);
    }

    // Build unified path: file/container/symbol
    let unified = if let Some(c) = container {
        format!("{}/{}/{}", file, c, symbol)
    } else {
        format!("{}/{}", file, symbol)
    };

    glob_match(&norm_pattern, &unified)
}

/// Simple glob matching: `*` matches any segment chars, `*/` matches one path segment.
fn glob_match(pattern: &str, text: &str) -> bool {
    // Use the glob crate-style matching with ** = any path segments
    let pat = glob::Pattern::new(pattern);
    match pat {
        Ok(p) => {
            let opts = glob::MatchOptions {
                case_sensitive: true,
                require_literal_separator: false,
                require_literal_leading_dot: false,
            };
            p.matches_with(text, opts)
        }
        Err(_) => text.contains(pattern),
    }
}

// ── Grouping ──────────────────────────────────────────────────────────────────

/// XOR-fold a token sequence into a single hash.
fn xor_fold(tokens: &[u64]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tokens.len().hash(&mut h);
    for t in tokens {
        t.hash(&mut h);
    }
    h.finish()
}

/// Group fragments by exact hash.
fn group_exact(fragments: &[Fragment]) -> Vec<Vec<usize>> {
    let mut groups: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, frag) in fragments.iter().enumerate() {
        let h = xor_fold(&frag.tokens);
        groups.entry(h).or_default().push(i);
    }
    groups.into_values().collect()
}

struct FuzzyGroups {
    clusters: Vec<Vec<usize>>,
    pair_sims: HashMap<(usize, usize), f64>,
}

/// Group fragments by MinHash + LSH + union-find clustering.
fn group_fuzzy(fragments: &[Fragment], similarity: f64) -> FuzzyGroups {
    let sigs: Vec<[u64; MINHASH_N]> = fragments
        .par_iter()
        .map(|f| compute_minhash(&f.tokens))
        .collect();

    // LSH bucketing
    let band_candidates: Vec<Vec<(usize, usize)>> = (0..LSH_BANDS)
        .into_par_iter()
        .map(|band| {
            let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
            for (idx, sig) in sigs.iter().enumerate() {
                let bh = lsh_band_hash(sig, band);
                buckets.entry(bh).or_default().push(idx);
            }
            let mut pairs = Vec::new();
            for bucket in buckets.values() {
                if bucket.len() < 2 {
                    continue;
                }
                for i in 0..bucket.len() {
                    for j in i + 1..bucket.len() {
                        let (a, b) = (bucket[i].min(bucket[j]), bucket[i].max(bucket[j]));
                        pairs.push((a, b));
                    }
                }
            }
            pairs
        })
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for band_pairs in band_candidates {
        for pair in band_pairs {
            if seen.insert(pair) {
                candidates.push(pair);
            }
        }
    }

    let mut uf = UnionFind::new(fragments.len());
    let mut pair_sims: HashMap<(usize, usize), f64> = HashMap::new();
    for (a, b) in &candidates {
        let sim = jaccard_estimate(&sigs[*a], &sigs[*b]);
        if sim >= similarity {
            uf.union(*a, *b);
            pair_sims.insert((*a, *b), sim);
        }
    }

    let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..fragments.len() {
        let root_id = uf.find(i);
        clusters.entry(root_id).or_default().push(i);
    }

    FuzzyGroups {
        clusters: clusters.into_values().collect(),
        pair_sims,
    }
}

/// Build a FragmentCluster from a set of fragment indices.
fn build_cluster(
    fragments: &[Fragment],
    indices: &[usize],
    is_fuzzy: bool,
    pair_sims: &HashMap<(usize, usize), f64>,
) -> FragmentCluster {
    let mut members: Vec<FragmentLocation> = indices
        .iter()
        .map(|&i| {
            let f = &fragments[i];
            FragmentLocation {
                file: f.file.clone(),
                start_line: f.start_line,
                end_line: f.end_line,
                symbol: f.symbol.clone(),
            }
        })
        .collect();
    members.sort_by(|a, b| a.file.cmp(&b.file).then(a.start_line.cmp(&b.start_line)));

    let frequency = members.len();
    let total_line_count: usize = members
        .iter()
        .map(|m| m.end_line.saturating_sub(m.start_line) + 1)
        .sum();
    let avg_lines = total_line_count as f64 / frequency as f64;
    let total_lines = (frequency as f64 * avg_lines) as usize;

    // Dominant node kind from first member
    let node_kind = fragments[indices[0]].node_kind.clone();

    // Aggregate kind counts across members
    let mut agg: HashMap<String, usize> = HashMap::new();
    for &i in indices {
        for (k, c) in &fragments[i].kind_counts {
            *agg.entry(k.clone()).or_default() += c;
        }
    }
    let mut label: Vec<(String, usize)> = agg.into_iter().collect();
    label.sort_by(|a, b| b.1.cmp(&a.1));
    label.truncate(5);

    // Hash representative
    let hash = format!("{:016x}", xor_fold(&fragments[indices[0]].tokens));

    // Compute average pairwise similarity in fuzzy mode
    let avg_similarity = if is_fuzzy && indices.len() >= 2 {
        let mut sim_sum = 0.0;
        let mut sim_count = 0usize;
        for i in 0..indices.len() {
            for j in i + 1..indices.len() {
                let (a, b) = (indices[i].min(indices[j]), indices[i].max(indices[j]));
                if let Some(&sim) = pair_sims.get(&(a, b)) {
                    sim_sum += sim;
                } else {
                    // Pair not in LSH candidates but in same cluster via transitivity;
                    // compute on demand from tokens via xor_fold Jaccard would be wrong
                    // (we don't have MinHash sigs here). Use 0.0 as lower bound.
                }
                sim_count += 1;
            }
        }
        Some(if sim_count > 0 {
            sim_sum / sim_count as f64
        } else {
            0.0
        })
    } else {
        None
    };

    FragmentCluster {
        hash,
        frequency,
        total_lines,
        avg_lines,
        node_kind,
        label,
        avg_similarity,
        members,
    }
}

// ── OutputFormatter ───────────────────────────────────────────────────────────

impl OutputFormatter for FragmentsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "# Fragment Analysis ({} fragments → {} clusters, {} unclustered, min_nodes={}, inline_depth={})",
            self.total_fragments_scanned, self.total_clusters, self.unclustered_count, self.min_nodes, self.inline_depth
        ));
        lines.push(String::new());

        if self.clusters.is_empty() {
            lines.push("No repeated fragments found.".to_string());
            return lines.join("\n");
        }

        // Header
        lines.push(format!(
            "{:<18} {:>5} {:>10} {:>10}  {}",
            "Hash", "Freq", "TotalLn", "AvgLn", "Kind / Label"
        ));
        lines.push("-".repeat(72));

        for cluster in &self.clusters {
            let label_str: String = cluster
                .label
                .iter()
                .take(3)
                .map(|(k, c)| format!("{} ×{}", k, c))
                .collect::<Vec<_>>()
                .join(", ");
            let sim_str = cluster
                .avg_similarity
                .map(|s| format!("  sim={:.0}%", s * 100.0))
                .unwrap_or_default();
            lines.push(format!(
                "{:<18} {:>5} {:>10} {:>10.1}  {} [{}]{}",
                &cluster.hash[..12],
                cluster.frequency,
                cluster.total_lines,
                cluster.avg_lines,
                cluster.node_kind,
                label_str,
                sim_str,
            ));

            // Top 3 locations
            for loc in cluster.members.iter().take(3) {
                let sym = loc.symbol.as_deref().unwrap_or("-");
                lines.push(format!(
                    "  {}:{}-{} ({})",
                    loc.file, loc.start_line, loc.end_line, sym
                ));
            }
            if cluster.members.len() > 3 {
                lines.push(format!("  ... and {} more", cluster.members.len() - 3));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "\x1b[1m# Fragment Analysis\x1b[0m ({} fragments → \x1b[36m{}\x1b[0m clusters, {} unclustered, min_nodes={}, inline_depth={})",
            self.total_fragments_scanned, self.total_clusters, self.unclustered_count, self.min_nodes, self.inline_depth
        ));
        lines.push(String::new());

        if self.clusters.is_empty() {
            lines.push("No repeated fragments found.".to_string());
            return lines.join("\n");
        }

        for (i, cluster) in self.clusters.iter().enumerate() {
            let label_str: String = cluster
                .label
                .iter()
                .take(3)
                .map(|(k, c)| format!("\x1b[36m{}\x1b[0m ×{}", k, c))
                .collect::<Vec<_>>()
                .join(", ");
            let sim_str = cluster
                .avg_similarity
                .map(|s| format!("  sim=\x1b[32m{:.0}%\x1b[0m", s * 100.0))
                .unwrap_or_default();
            lines.push(format!(
                "\x1b[1;33m#{}\x1b[0m \x1b[2m{}\x1b[0m  freq=\x1b[1m{}\x1b[0m  lines={}  avg={:.1}  \x1b[35m{}\x1b[0m [{}]{}",
                i + 1,
                &cluster.hash[..12],
                cluster.frequency,
                cluster.total_lines,
                cluster.avg_lines,
                cluster.node_kind,
                label_str,
                sim_str,
            ));

            for loc in cluster.members.iter().take(3) {
                let sym = loc.symbol.as_deref().unwrap_or("-");
                lines.push(format!(
                    "  \x1b[34m{}:{}-{}\x1b[0m ({})",
                    loc.file, loc.start_line, loc.end_line, sym
                ));
            }
            if cluster.members.len() > 3 {
                lines.push(format!(
                    "  \x1b[2m... and {} more\x1b[0m",
                    cluster.members.len() - 3
                ));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
}
