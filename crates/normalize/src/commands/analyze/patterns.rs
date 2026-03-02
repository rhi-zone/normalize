//! Pattern catalog — auto-detect recurring structural code patterns.
//!
//! Finds functions with the same control-flow skeleton even when the actual code
//! is completely different. Uses MinHash + LSH from `duplicates.rs` on structural
//! tokens (control-flow nodes, calls, assignments) instead of full AST tokens.

use crate::extract::Extractor;
use crate::output::OutputFormatter;
use normalize_languages::support_for_path;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use super::duplicates::{
    LSH_BANDS, MINHASH_N, compute_minhash, find_function_node, flatten_symbols, jaccard_estimate,
    lsh_band_hash,
};

// ── Data structures ───────────────────────────────────────────────────────────

/// A function belonging to a pattern cluster.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct PatternMember {
    pub file: String,
    pub symbol: String,
    pub start_line: usize,
    pub end_line: usize,
    pub line_count: usize,
}

/// A cluster of functions sharing a structural pattern.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct PatternCluster {
    pub label: String,
    pub members: Vec<PatternMember>,
    pub frequency: usize,
    pub avg_similarity: f64,
    /// Dominant structural elements: `[("if", 3), ("call", 5)]`
    pub structural_elements: Vec<(String, usize)>,
}

/// Full patterns analysis report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PatternsReport {
    pub patterns: Vec<PatternCluster>,
    pub total_functions: usize,
    pub total_patterns: usize,
    pub unclustered_functions: usize,
}

// ── Union-Find ────────────────────────────────────────────────────────────────

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

// ── Structural tokenization ───────────────────────────────────────────────────

/// Walk an AST node and emit hashes only for structural (control-flow, call,
/// assignment) node kinds. Everything else is ignored.
fn serialize_structural_tokens(
    node: &tree_sitter::Node,
    structural_kinds: &HashSet<&str>,
    out: &mut Vec<u64>,
) {
    let kind = node.kind();

    let is_structural =
        structural_kinds.contains(kind) || kind.contains("call") || kind.contains("assignment");

    if is_structural {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        kind.hash(&mut h);
        out.push(h.finish());
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            serialize_structural_tokens(&cursor.node(), structural_kinds, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Categorize a node kind into a readable label for pattern naming.
fn categorize_kind(kind: &str) -> &str {
    if kind.contains("if") || kind.contains("match") || kind.contains("switch") {
        "branch"
    } else if kind.contains("for") || kind.contains("while") || kind.contains("loop") {
        "loop"
    } else if kind.contains("try") || kind.contains("catch") || kind.contains("rescue") {
        "error-handling"
    } else if kind.contains("return") || kind.contains("break") || kind.contains("continue") {
        "exit"
    } else if kind.contains("call") {
        "call"
    } else if kind.contains("assignment") {
        "transform"
    } else {
        "control"
    }
}

/// Generate a human-readable label from structural element counts.
fn generate_pattern_label(elements: &[(String, usize)]) -> String {
    if elements.is_empty() {
        return "unknown".to_string();
    }

    // Categorize and aggregate
    let mut categories: HashMap<&str, usize> = HashMap::new();
    for (kind, count) in elements {
        let cat = categorize_kind(kind);
        *categories.entry(cat).or_default() += count;
    }

    // Sort by count descending, pick top 2
    let mut sorted: Vec<(&str, usize)> = categories.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let parts: Vec<&str> = sorted.iter().take(2).map(|(cat, _)| *cat).collect();

    match parts.len() {
        0 => "unknown".to_string(),
        1 => {
            let total: usize = elements.iter().map(|(_, c)| c).sum();
            if total > 6 {
                format!("{}-heavy", parts[0])
            } else {
                parts[0].to_string()
            }
        }
        _ => format!("{}-{}", parts[0], parts[1]),
    }
}

// ── Core analysis ─────────────────────────────────────────────────────────────

/// Analyze structural patterns across all source files under `root`.
pub fn analyze_patterns(
    root: &PathBuf,
    similarity: f64,
    min_members: usize,
    limit: usize,
    exclude: &[String],
    only: &[String],
) -> Result<PatternsReport, String> {
    let extractor = Extractor::new();
    let filter = crate::commands::build_filter(root, exclude, only);

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

    // Per-function: (file, symbol, start_line, end_line, structural_kinds, minhash)
    type FnEntry = (String, String, usize, usize, Vec<String>, [u64; MINHASH_N]);

    let per_file: Vec<Vec<FnEntry>> = files
        .par_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            let lang = support_for_path(path)?;
            let tree = crate::parsers::parse_with_grammar(lang.grammar_name(), &content)?;
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string();

            // Build structural kinds set from language traits
            let structural_kinds: HashSet<&str> = lang
                .control_flow_kinds()
                .iter()
                .chain(lang.complexity_nodes().iter())
                .copied()
                .collect();

            let result = extractor.extract(path, &content);
            let mut entries = Vec::new();

            for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
                let kind = sym.kind.as_str();
                if kind != "function" && kind != "method" {
                    continue;
                }

                if let Some(node) = find_function_node(&tree, sym.start_line) {
                    let mut tokens = Vec::new();
                    serialize_structural_tokens(&node, &structural_kinds, &mut tokens);

                    // Skip trivial functions with < 3 structural tokens
                    if tokens.len() < 3 {
                        continue;
                    }

                    // Collect the actual node kind strings for labeling
                    let mut kind_counts: HashMap<String, usize> = HashMap::new();
                    collect_structural_kinds(&node, &structural_kinds, &mut kind_counts);
                    let mut kinds_vec: Vec<(String, usize)> = kind_counts.into_iter().collect();
                    kinds_vec.sort_by(|a, b| b.1.cmp(&a.1));
                    let kind_names: Vec<String> =
                        kinds_vec.iter().map(|(k, _)| k.clone()).collect();

                    let sig = compute_minhash(&tokens);
                    entries.push((
                        rel_path.clone(),
                        sym.name.clone(),
                        sym.start_line,
                        sym.end_line,
                        kind_names,
                        sig,
                    ));
                }
            }

            if entries.is_empty() {
                None
            } else {
                Some(entries)
            }
        })
        .collect();

    let all_fns: Vec<FnEntry> = per_file.into_iter().flatten().collect();
    let total_functions = all_fns.len();

    if total_functions == 0 {
        return Ok(PatternsReport {
            patterns: Vec::new(),
            total_functions: 0,
            total_patterns: 0,
            unclustered_functions: 0,
        });
    }

    // LSH bucketing → candidate pairs
    let band_candidates: Vec<Vec<(usize, usize)>> = (0..LSH_BANDS)
        .into_par_iter()
        .map(|band| {
            let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
            for (idx, (_, _, _, _, _, sig)) in all_fns.iter().enumerate() {
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

    // Merge and deduplicate
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for band_pairs in band_candidates {
        for pair in band_pairs {
            if seen.insert(pair) {
                candidates.push(pair);
            }
        }
    }

    // Filter by Jaccard similarity threshold, union-find clustering
    let mut uf = UnionFind::new(total_functions);
    let mut pair_similarities: HashMap<(usize, usize), f64> = HashMap::new();

    for (a, b) in &candidates {
        let sim = jaccard_estimate(&all_fns[*a].5, &all_fns[*b].5);
        if sim >= similarity {
            uf.union(*a, *b);
            pair_similarities.insert((*a, *b), sim);
        }
    }

    // Group by cluster root
    let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..total_functions {
        let root_id = uf.find(i);
        clusters.entry(root_id).or_default().push(i);
    }

    // Build pattern clusters
    let mut patterns: Vec<PatternCluster> = Vec::new();
    for members in clusters.values() {
        if members.len() < min_members {
            continue;
        }

        // Compute average similarity within the cluster
        let mut sim_sum = 0.0;
        let mut sim_count = 0;
        for i in 0..members.len() {
            for j in i + 1..members.len() {
                let (a, b) = (members[i].min(members[j]), members[i].max(members[j]));
                if let Some(sim) = pair_similarities.get(&(a, b)) {
                    sim_sum += sim;
                    sim_count += 1;
                } else {
                    // Compute on demand for pairs not in candidates
                    let sim = jaccard_estimate(&all_fns[a].5, &all_fns[b].5);
                    sim_sum += sim;
                    sim_count += 1;
                }
            }
        }
        let avg_similarity = if sim_count > 0 {
            sim_sum / sim_count as f64
        } else {
            0.0
        };

        // Aggregate structural elements across all members
        let mut element_counts: HashMap<String, usize> = HashMap::new();
        for &idx in members {
            for kind in &all_fns[idx].4 {
                *element_counts.entry(kind.clone()).or_default() += 1;
            }
        }
        let mut structural_elements: Vec<(String, usize)> = element_counts.into_iter().collect();
        structural_elements.sort_by(|a, b| b.1.cmp(&a.1));

        let label = generate_pattern_label(&structural_elements);

        let mut pattern_members: Vec<PatternMember> = members
            .iter()
            .map(|&idx| {
                let (file, symbol, start_line, end_line, _, _) = &all_fns[idx];
                PatternMember {
                    file: file.clone(),
                    symbol: symbol.clone(),
                    start_line: *start_line,
                    end_line: *end_line,
                    line_count: end_line.saturating_sub(*start_line) + 1,
                }
            })
            .collect();
        pattern_members.sort_by(|a, b| a.file.cmp(&b.file).then(a.start_line.cmp(&b.start_line)));

        let frequency = pattern_members.len();
        patterns.push(PatternCluster {
            label,
            members: pattern_members,
            frequency,
            avg_similarity,
            structural_elements,
        });
    }

    // Sort by frequency descending
    patterns.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    patterns.truncate(limit);

    // Deduplicate labels by appending a numeric suffix
    let mut label_counts: HashMap<String, usize> = HashMap::new();
    for pattern in &mut patterns {
        let count = label_counts.entry(pattern.label.clone()).or_default();
        *count += 1;
        if *count > 1 {
            pattern.label = format!("{}-{}", pattern.label, count);
        }
    }

    let clustered: usize = clusters
        .values()
        .filter(|m| m.len() >= min_members)
        .map(|m| m.len())
        .sum();

    Ok(PatternsReport {
        total_patterns: patterns.len(),
        unclustered_functions: total_functions - clustered,
        patterns,
        total_functions,
    })
}

/// Collect structural node kind counts from an AST subtree.
fn collect_structural_kinds(
    node: &tree_sitter::Node,
    structural_kinds: &HashSet<&str>,
    counts: &mut HashMap<String, usize>,
) {
    let kind = node.kind();
    if structural_kinds.contains(kind) || kind.contains("call") || kind.contains("assignment") {
        *counts.entry(kind.to_string()).or_default() += 1;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_structural_kinds(&cursor.node(), structural_kinds, counts);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── OutputFormatter ───────────────────────────────────────────────────────────

impl OutputFormatter for PatternsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "# Pattern Catalog ({} functions → {} patterns, {} unclustered)",
            self.total_functions, self.total_patterns, self.unclustered_functions
        ));
        lines.push(String::new());

        if self.patterns.is_empty() {
            lines.push("No structural patterns detected.".to_string());
            return lines.join("\n");
        }

        for (i, pattern) in self.patterns.iter().enumerate() {
            lines.push(format!(
                "Pattern {}: \"{}\" ({} functions, avg similarity {:.0}%)",
                i + 1,
                pattern.label,
                pattern.frequency,
                pattern.avg_similarity * 100.0
            ));

            // Show top structural elements
            let elements: Vec<String> = pattern
                .structural_elements
                .iter()
                .take(5)
                .map(|(kind, count)| format!("{} ×{}", kind, count))
                .collect();
            if !elements.is_empty() {
                lines.push(format!("  Structural elements: {}", elements.join(", ")));
            }

            // Show up to 3 example members
            lines.push("  Examples:".to_string());
            for member in pattern.members.iter().take(3) {
                lines.push(format!(
                    "    {}:{} (L{}-{})",
                    member.file, member.symbol, member.start_line, member.end_line
                ));
            }
            if pattern.members.len() > 3 {
                lines.push(format!("    ... and {} more", pattern.members.len() - 3));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "\x1b[1m# Pattern Catalog\x1b[0m ({} functions → \x1b[36m{}\x1b[0m patterns, {} unclustered)",
            self.total_functions, self.total_patterns, self.unclustered_functions
        ));
        lines.push(String::new());

        if self.patterns.is_empty() {
            lines.push("No structural patterns detected.".to_string());
            return lines.join("\n");
        }

        for (i, pattern) in self.patterns.iter().enumerate() {
            lines.push(format!(
                "\x1b[1;33mPattern {}\x1b[0m: \x1b[1m\"{}\"\x1b[0m ({} functions, avg similarity \x1b[32m{:.0}%\x1b[0m)",
                i + 1,
                pattern.label,
                pattern.frequency,
                pattern.avg_similarity * 100.0
            ));

            let elements: Vec<String> = pattern
                .structural_elements
                .iter()
                .take(5)
                .map(|(kind, count)| format!("\x1b[36m{}\x1b[0m ×{}", kind, count))
                .collect();
            if !elements.is_empty() {
                lines.push(format!("  Structural elements: {}", elements.join(", ")));
            }

            lines.push("  Examples:".to_string());
            for member in pattern.members.iter().take(3) {
                lines.push(format!(
                    "    \x1b[34m{}:{}\x1b[0m (L{}-{})",
                    member.file, member.symbol, member.start_line, member.end_line
                ));
            }
            if pattern.members.len() > 3 {
                lines.push(format!(
                    "    \x1b[2m... and {} more\x1b[0m",
                    pattern.members.len() - 3
                ));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
}
