//! Pattern catalog — auto-detect recurring structural code patterns.
//!
//! Finds functions with the same control-flow skeleton even when the actual code
//! is completely different. Uses MinHash + LSH from `normalize-code-similarity` on structural
//! tokens (control-flow nodes, calls, assignments) instead of full AST tokens.

use crate::extract::Extractor;
use crate::output::OutputFormatter;
use normalize_code_similarity::{
    LSH_BANDS, MINHASH_N, UnionFind, collect_structural_kinds, compute_minhash, find_function_node,
    flatten_symbols, generate_pattern_label, jaccard_estimate, lsh_band_hash,
    serialize_structural_tokens,
};
use normalize_languages::{parsers::grammar_loader, support_for_path};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use streaming_iterator::StreamingIterator;

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

/// Run the complexity query on a function node and return the kinds of all matched nodes.
fn complexity_node_kinds(
    fn_node: &tree_sitter::Node,
    grammar: tree_sitter::Language,
    query_str: &str,
    source: &[u8],
) -> HashSet<String> {
    let Ok(query) = tree_sitter::Query::new(&grammar, query_str) else {
        return HashSet::new();
    };
    let complexity_idx = query.capture_index_for_name("complexity");
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut kinds = HashSet::new();
    let mut matches = cursor.matches(&query, *fn_node, source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if complexity_idx.is_some_and(|i| i == cap.index) {
                kinds.insert(cap.node.kind().to_string());
            }
        }
    }
    kinds
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

            // Load complexity query for this language (used per-function below)
            let loader = grammar_loader();
            let complexity_query = loader.get_complexity(lang.grammar_name());
            let grammar = loader.get(lang.grammar_name());

            let result = extractor.extract(path, &content);
            let mut entries = Vec::new();

            for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
                let kind = sym.kind.as_str();
                if kind != "function" && kind != "method" {
                    continue;
                }

                if let Some(node) = find_function_node(&tree, sym.start_line) {
                    // Build complexity node kind set from query, or empty if unavailable
                    let complexity_kinds_owned = match (&complexity_query, &grammar) {
                        (Some(q), Some(g)) => {
                            complexity_node_kinds(&node, g.clone(), q, content.as_bytes())
                        }
                        _ => HashSet::new(),
                    };
                    let complexity_kinds: HashSet<&str> =
                        complexity_kinds_owned.iter().map(|s| s.as_str()).collect();

                    let mut tokens = Vec::new();
                    serialize_structural_tokens(&node, &complexity_kinds, &mut tokens);

                    // Skip trivial functions with < 5 structural tokens
                    if tokens.len() < 5 {
                        continue;
                    }

                    // Collect the actual node kind strings for labeling
                    let mut kind_counts: HashMap<String, usize> = HashMap::new();
                    collect_structural_kinds(&node, &complexity_kinds, &mut kind_counts);
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
    normalize_analyze::ranked::rank_and_truncate(
        &mut patterns,
        limit,
        |a, b| b.frequency.cmp(&a.frequency),
        |p| p.frequency as f64,
    );

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
