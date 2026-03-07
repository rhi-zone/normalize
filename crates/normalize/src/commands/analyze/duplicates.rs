//! Duplicate function and type detection.

use super::duplicates_views::DuplicatesReport;
use crate::extract::Extractor;
use crate::filter::Filter;
use crate::output::OutputFormatter;
use crate::parsers;
use normalize_code_similarity::{
    LSH_BANDS, MINHASH_N, SHINGLE_K, compute_function_hash, compute_minhash, find_function_node,
    flatten_symbols, jaccard_estimate, lsh_band_hash, serialize_subtree_tokens,
};
use normalize_languages::support_for_path;
use rayon::prelude::*;
use serde::Serialize;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A group of duplicate functions
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicateFunctionGroup {
    #[serde(serialize_with = "serialize_hash")]
    hash: u64,
    locations: Vec<DuplicateFunctionLocation>,
    line_count: usize,
}

fn serialize_hash<S>(hash: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{:016x}", hash))
}

/// Location of a duplicate function instance
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicateFunctionLocation {
    file: String,
    symbol: String,
    start_line: usize,
    end_line: usize,
}

/// Type information
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
struct TypeInfo {
    file: String,
    name: String,
    start_line: usize,
    fields: Vec<String>,
}

/// A pair of duplicate types
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DuplicatePair {
    type1: TypeInfo,
    type2: TypeInfo,
    overlap_percent: usize,
    common_fields: Vec<String>,
}

/// Duplicate types analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateTypesReport {
    files_scanned: usize,
    types_analyzed: usize,
    min_overlap_percent: usize,
    duplicates: Vec<DuplicatePair>,
}

impl OutputFormatter for DuplicateTypesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Duplicate Type Detection".to_string());
        lines.push(String::new());
        lines.push(format!("Files scanned: {}", self.files_scanned));
        lines.push(format!("Types analyzed: {}", self.types_analyzed));
        lines.push(format!("Duplicate pairs: {}", self.duplicates.len()));
        lines.push(format!("Min overlap: {}%", self.min_overlap_percent));
        lines.push(String::new());

        if self.duplicates.is_empty() {
            lines.push("No duplicate types detected.".to_string());
        } else {
            lines.push("Potential Duplicates (sorted by overlap):".to_string());
            lines.push(String::new());

            for (i, dup) in self.duplicates.iter().take(20).enumerate() {
                lines.push(format!(
                    "{}. {}% overlap ({} common fields):",
                    i + 1,
                    dup.overlap_percent,
                    dup.common_fields.len()
                ));
                lines.push(format!(
                    "   {} ({}:{}) - {} fields",
                    dup.type1.name,
                    dup.type1.file,
                    dup.type1.start_line,
                    dup.type1.fields.len()
                ));
                lines.push(format!(
                    "   {} ({}:{}) - {} fields",
                    dup.type2.name,
                    dup.type2.file,
                    dup.type2.start_line,
                    dup.type2.fields.len()
                ));
                lines.push(format!("   Common: {}", dup.common_fields.join(", ")));
                lines.push(String::new());
            }

            if self.duplicates.len() > 20 {
                lines.push(format!("... and {} more pairs", self.duplicates.len() - 20));
            }

            lines.push(String::new());
            lines.push("To suppress: normalize analyze duplicate-types --allow TypeName1 TypeName2 --reason \"explanation\"".to_string());
        }

        lines.join("\n")
    }
}

/// Load allowed duplicate function locations from .normalize/duplicate-functions-allow file
fn load_duplicate_functions_allowlist(root: &Path) -> HashSet<String> {
    let path = root.join(".normalize/duplicate-functions-allow");
    let mut allowed = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            allowed.insert(line.to_string());
        }
    }
    allowed
}

/// Detect duplicate functions.
pub struct DuplicateFunctionsConfig<'a> {
    pub roots: &'a [PathBuf],
    pub elide_identifiers: bool,
    pub elide_literals: bool,
    pub show_source: bool,
    pub min_lines: usize,
    pub include_trait_impls: bool,
    pub filter: Option<&'a Filter>,
}

// ── Duplicate block detection (subtree-level) ─────────────────────────────────

/// A location of a duplicate block instance
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct DuplicateBlockLocation {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// A group of duplicate blocks
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateBlockGroup {
    #[serde(serialize_with = "serialize_hash")]
    hash: u64,
    pub locations: Vec<DuplicateBlockLocation>,
    pub line_count: usize,
}

/// Walk every node in the tree and hash subtrees at or above min_lines.
fn is_function_kind(kind: &str) -> bool {
    kind.contains("function") || kind.contains("method")
}

// ── Allow-file helpers ────────────────────────────────────────────────────────

/// Find the innermost function/method symbol that contains `line` in `result`.
fn containing_function(
    result: &normalize_facts::extract::ExtractResult,
    line: usize,
) -> Option<String> {
    let mut best: Option<(usize, &str)> = None; // (start_line, name)
    for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
        let kind = sym.kind.as_str();
        if kind != "function" && kind != "method" {
            continue;
        }
        if sym.start_line <= line && line <= sym.end_line {
            // Prefer the innermost (latest start line).
            // normalize-syntax-allow: rust/unwrap-in-impl - guarded by is_none() check in same condition
            if best.is_none() || sym.start_line > best.unwrap().0 {
                best = Some((sym.start_line, &sym.name));
            }
        }
    }
    best.map(|(_, name)| name.to_string())
}

/// Format a block location as an allow-file key.
/// If the block is inside a named function: `file:func:start-end`
/// Otherwise: `file:start-end`
fn block_allow_key(
    file: &str,
    start_line: usize,
    end_line: usize,
    containing_fn: Option<&str>,
) -> String {
    match containing_fn {
        Some(func) => format!("{}:{}:{}-{}", file, func, start_line, end_line),
        None => format!("{}:{}-{}", file, start_line, end_line),
    }
}

fn load_block_allowlist(root: &Path, filename: &str) -> HashSet<String> {
    let path = root.join(".normalize").join(filename);
    let mut allowed = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            allowed.insert(line.to_string());
        }
    }
    allowed
}

#[allow(clippy::too_many_arguments)]
fn collect_block_hashes(
    node: &tree_sitter::Node,
    content: &[u8],
    file: &str,
    min_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    skip_functions: bool,
    out: &mut HashMap<u64, Vec<DuplicateBlockLocation>>,
) {
    let kind = node.kind();

    if skip_functions && is_function_kind(kind) {
        return;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let line_count = end_line.saturating_sub(start_line) + 1;

    if line_count >= min_lines {
        let hash = compute_function_hash(node, content, elide_identifiers, elide_literals);
        out.entry(hash).or_default().push(DuplicateBlockLocation {
            file: file.to_string(),
            start_line,
            end_line,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_block_hashes(
            &child,
            content,
            file,
            min_lines,
            elide_identifiers,
            elide_literals,
            skip_functions,
            out,
        );
    }
}

/// After bucketing, suppress groups whose locations are fully contained inside
/// a location from a larger group (in the same file). Returns filtered groups.
fn suppress_contained_blocks(mut groups: Vec<DuplicateBlockGroup>) -> Vec<DuplicateBlockGroup> {
    // Sort largest first so we process parents before children.
    groups.sort_by(|a, b| b.line_count.cmp(&a.line_count));

    // Collect all "taken" ranges per file from already-accepted groups.
    let mut taken: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    let mut result = Vec::new();

    for group in groups {
        let mut kept = Vec::new();
        for loc in &group.locations {
            let ranges = taken.entry(loc.file.clone()).or_default();
            let contained = ranges
                .iter()
                .any(|&(s, e)| s <= loc.start_line && loc.end_line <= e);
            if !contained {
                kept.push(loc.clone());
            }
        }
        if kept.len() >= 2 {
            // Register these locations as taken.
            for loc in &kept {
                taken
                    .entry(loc.file.clone())
                    .or_default()
                    .push((loc.start_line, loc.end_line));
            }
            result.push(DuplicateBlockGroup {
                hash: group.hash,
                locations: kept,
                line_count: group.line_count,
            });
        }
    }

    result
}

pub struct DuplicateBlocksConfig<'a> {
    pub root: &'a Path,
    pub min_lines: usize,
    pub elide_identifiers: bool,
    pub elide_literals: bool,
    pub skip_functions: bool,
    pub show_source: bool,
    pub allow: Option<String>,
    pub reason: Option<String>,
    pub filter: Option<&'a Filter>,
}

// ── Fuzzy / partial clone detection (MinHash LSH) ─────────────────────────────

/// A pair of similar (but not necessarily identical) blocks.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimilarBlockPair {
    pub location_a: DuplicateBlockLocation,
    pub location_b: DuplicateBlockLocation,
    /// Estimated Jaccard similarity of their AST token shingles (0.0–1.0)
    pub similarity: f64,
    pub line_count: usize,
}

/// Collect all subtrees above min_lines, returning (location, minhash signature).
#[allow(clippy::too_many_arguments)]
fn collect_block_signatures(
    node: &tree_sitter::Node,
    content: &[u8],
    file: &str,
    min_lines: usize,
    elide_identifiers: bool,
    elide_literals: bool,
    skeleton: bool,
    out: &mut Vec<(DuplicateBlockLocation, [u64; MINHASH_N])>,
) {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let line_count = end_line.saturating_sub(start_line) + 1;

    if line_count >= min_lines {
        let mut tokens = Vec::new();
        serialize_subtree_tokens(
            node,
            content,
            elide_identifiers,
            elide_literals,
            skeleton,
            &mut tokens,
        );
        // In skeleton mode a complex function can reduce to very few tokens,
        // or to a long repetitive sequence (e.g. many match arms all elided).
        // Both cases produce false positives — filter them out.
        let min_tokens = if skeleton { SHINGLE_K * 4 } else { SHINGLE_K };
        let skip = if tokens.len() < min_tokens {
            true
        } else if skeleton {
            // Require at least 30% unique tokens: repetitive skeletons
            // (long match statements, uniform loop bodies) are not useful signal.
            let unique = tokens
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len();
            (unique as f64 / tokens.len() as f64) < 0.3
        } else {
            false
        };

        if !skip {
            let sig = compute_minhash(&tokens);
            out.push((
                DuplicateBlockLocation {
                    file: file.to_string(),
                    start_line,
                    end_line,
                },
                sig,
            ));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_block_signatures(
            &child,
            content,
            file,
            min_lines,
            elide_identifiers,
            elide_literals,
            skeleton,
            out,
        );
    }
}

/// Suppress overlapping pairs: if two pairs involve the same two files and
/// both locations significantly overlap, keep only the largest (first seen,
/// since pairs are pre-sorted by similarity desc then size desc).
fn suppress_overlapping_pairs(pairs: Vec<SimilarBlockPair>) -> Vec<SimilarBlockPair> {
    // Track accepted ranges per file.
    let mut taken: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut result = Vec::new();

    for pair in pairs {
        let taken_a = taken.entry(pair.location_a.file.clone()).or_default();
        let overlaps_a = taken_a.iter().any(|&(s, e)| {
            overlap_ratio(s, e, pair.location_a.start_line, pair.location_a.end_line) > 0.5
        });

        let taken_b = taken.entry(pair.location_b.file.clone()).or_default();
        let overlaps_b = taken_b.iter().any(|&(s, e)| {
            overlap_ratio(s, e, pair.location_b.start_line, pair.location_b.end_line) > 0.5
        });

        if overlaps_a && overlaps_b {
            continue;
        }

        taken
            .entry(pair.location_a.file.clone())
            .or_default()
            .push((pair.location_a.start_line, pair.location_a.end_line));
        taken
            .entry(pair.location_b.file.clone())
            .or_default()
            .push((pair.location_b.start_line, pair.location_b.end_line));

        result.push(pair);
    }

    result
}

fn overlap_ratio(s1: usize, e1: usize, s2: usize, e2: usize) -> f64 {
    let overlap_start = s1.max(s2);
    let overlap_end = e1.min(e2);
    if overlap_end < overlap_start {
        return 0.0;
    }
    let overlap = (overlap_end - overlap_start + 1) as f64;
    let union = (e1.max(e2) - s1.min(s2) + 1) as f64;
    overlap / union
}

pub struct SimilarBlocksConfig<'a> {
    pub root: &'a Path,
    pub min_lines: usize,
    pub similarity: f64,
    pub elide_identifiers: bool,
    pub elide_literals: bool,
    pub skeleton: bool,
    pub show_source: bool,
    pub include_trait_impls: bool,
    pub allow: Option<String>,
    pub reason: Option<String>,
    pub filter: Option<&'a Filter>,
}

// ── Similar functions (fuzzy function-level matching) ─────────────────────────

/// A pair of similar functions.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimilarFunctionPair {
    pub file_a: String,
    pub symbol_a: String,
    pub start_line_a: usize,
    pub end_line_a: usize,
    pub file_b: String,
    pub symbol_b: String,
    pub start_line_b: usize,
    pub end_line_b: usize,
    pub similarity: f64,
    pub line_count: usize,
}

pub struct SimilarFunctionsConfig<'a> {
    pub roots: &'a [PathBuf],
    pub min_lines: usize,
    pub similarity: f64,
    pub elide_identifiers: bool,
    pub elide_literals: bool,
    pub skeleton: bool,
    pub show_source: bool,
    pub include_trait_impls: bool,
    pub allow: Option<String>,
    pub reason: Option<String>,
    pub filter: Option<&'a Filter>,
}

/// Core pair detection: extract MinHash signatures and find similar function pairs via LSH.
/// Result of similar-function detection including per-file function counts.
pub(crate) struct SimilarFunctionPairsResult {
    pub files_scanned: usize,
    pub functions_analyzed: usize,
    pub pairs: Vec<SimilarFunctionPair>,
    /// Per-file total function counts and line counts (relative path, fn_count, line_count).
    /// Includes all functions, not just those above min_lines. Useful for computing
    /// uniqueness ratios without a redundant file walk.
    pub file_fn_counts: Vec<(String, usize, usize)>,
}

///
/// Returns similar function pairs plus per-file function counts. Pairs are sorted by
/// similarity descending. Used by both `cmd_similar_functions` and the clustering analysis.
#[allow(clippy::too_many_arguments)]
pub(crate) fn find_similar_function_pairs(
    roots: &[PathBuf],
    min_lines: usize,
    similarity: f64,
    elide_identifiers: bool,
    elide_literals: bool,
    skeleton: bool,
    include_trait_impls: bool,
    filter: Option<&crate::filter::Filter>,
) -> SimilarFunctionPairsResult {
    let extractor = Extractor::new();
    let multi_repo = roots.len() > 1;
    let mut all_fns: Vec<(String, String, usize, usize, [u64; MINHASH_N])> = Vec::new();
    let mut files_scanned = 0usize;
    let mut file_fn_counts: Vec<(String, usize, usize)> = Vec::new();

    for root in roots {
        let repo_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

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
                if let Some(f) = filter {
                    let rel_path = e.path().strip_prefix(root).unwrap_or(e.path());
                    f.matches(rel_path)
                } else {
                    true
                }
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        // Per-file: (total_fn_count, minhash_entries)
        // Per-file: (rel_path, total_fn_count, line_count, minhash_entries)
        #[allow(clippy::type_complexity)]
        let per_file: Vec<(
            String,
            usize,
            usize,
            Vec<(String, String, usize, usize, [u64; MINHASH_N])>,
        )> = files
            .par_iter()
            .filter_map(|path| {
                let content = std::fs::read_to_string(path).ok()?;
                let support = support_for_path(path)?;
                let tree = crate::parsers::parse_with_grammar(support.grammar_name(), &content)?;
                let root_rel = path.strip_prefix(root).unwrap_or(path);
                let rel_path = if multi_repo {
                    format!("{}/{}", repo_name, root_rel.display())
                } else {
                    root_rel.display().to_string()
                };

                let line_count = content.lines().count();
                let result = extractor.extract(path, &content);
                let mut entries = Vec::new();
                let mut total_fn_count = 0usize;

                for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
                    let kind = sym.kind.as_str();
                    if kind != "function" && kind != "method" {
                        continue;
                    }
                    total_fn_count += 1;

                    let line_count = sym.end_line.saturating_sub(sym.start_line) + 1;
                    if line_count < min_lines {
                        continue;
                    }

                    if let Some(node) = find_function_node(&tree, sym.start_line) {
                        let mut tokens = Vec::new();
                        serialize_subtree_tokens(
                            &node,
                            content.as_bytes(),
                            elide_identifiers,
                            elide_literals,
                            skeleton,
                            &mut tokens,
                        );
                        let min_tokens = if skeleton { SHINGLE_K * 4 } else { SHINGLE_K };
                        if tokens.len() < min_tokens {
                            continue;
                        }
                        if skeleton {
                            let unique = tokens
                                .iter()
                                .collect::<std::collections::HashSet<_>>()
                                .len();
                            if (unique as f64 / tokens.len() as f64) < 0.3 {
                                continue;
                            }
                        }
                        let sig = compute_minhash(&tokens);
                        entries.push((
                            rel_path.clone(),
                            sym.name.clone(),
                            sym.start_line,
                            sym.end_line,
                            sig,
                        ));
                    }
                }
                if total_fn_count == 0 {
                    None
                } else {
                    Some((rel_path, total_fn_count, line_count, entries))
                }
            })
            .collect();

        for (rel_path, fn_count, lines, entries) in per_file {
            files_scanned += 1;
            all_fns.extend(entries);
            file_fn_counts.push((rel_path, fn_count, lines));
        }
    }

    let functions_analyzed = all_fns.len();

    // LSH bucketing + candidate generation (parallelized per band).
    // Each band independently collects candidate pairs, then we merge and deduplicate.
    let band_candidates: Vec<Vec<(usize, usize)>> = (0..LSH_BANDS)
        .into_par_iter()
        .map(|band| {
            // Build buckets for this band only.
            let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
            for (idx, (_, _, _, _, sig)) in all_fns.iter().enumerate() {
                let bh = lsh_band_hash(sig, band);
                buckets.entry(bh).or_default().push(idx);
            }
            // Generate candidate pairs from buckets.
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

    // Merge and deduplicate candidates across bands.
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for band_pairs in band_candidates {
        for pair in band_pairs {
            if seen.insert(pair) {
                candidates.push(pair);
            }
        }
    }

    // Score and filter (parallel — can be 90K+ candidates).
    let mut pairs: Vec<SimilarFunctionPair> = candidates
        .into_par_iter()
        .filter_map(|(i, j)| {
            let (file_a, sym_a, start_a, end_a, sig_a) = &all_fns[i];
            let (file_b, sym_b, start_b, end_b, sig_b) = &all_fns[j];

            if file_a == file_b && start_a == start_b && end_a == end_b {
                return None;
            }

            if !skeleton {
                let len_a = end_a.saturating_sub(*start_a) + 1;
                let len_b = end_b.saturating_sub(*start_b) + 1;
                let ratio = len_a.min(len_b) as f64 / len_a.max(len_b) as f64;
                if ratio < 0.5 {
                    return None;
                }
            } else {
                let len_a = end_a.saturating_sub(*start_a) + 1;
                let len_b = end_b.saturating_sub(*start_b) + 1;
                let ratio = len_a.min(len_b) as f64 / len_a.max(len_b) as f64;
                if ratio < 0.2 {
                    return None;
                }
            }

            let sim = jaccard_estimate(sig_a, sig_b);
            if sim >= 1.0 || sim < similarity {
                return None;
            }

            let line_count = end_a
                .saturating_sub(*start_a)
                .max(end_b.saturating_sub(*start_b))
                + 1;

            Some(SimilarFunctionPair {
                file_a: file_a.clone(),
                symbol_a: sym_a.clone(),
                start_line_a: *start_a,
                end_line_a: *end_a,
                file_b: file_b.clone(),
                symbol_b: sym_b.clone(),
                start_line_b: *start_b,
                end_line_b: *end_b,
                similarity: sim,
                line_count,
            })
        })
        .collect();

    if !include_trait_impls {
        pairs.retain(|p| p.symbol_a != p.symbol_b);
    }

    pairs.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.line_count.cmp(&a.line_count))
    });

    SimilarFunctionPairsResult {
        files_scanned,
        functions_analyzed,
        pairs,
        file_fn_counts,
    }
}

/// Build duplicate functions report without printing (for service layer).
pub fn build_duplicate_functions_report(cfg: DuplicateFunctionsConfig<'_>) -> DuplicatesReport {
    use crate::commands::analyze::duplicates_views::{
        CodeLocation, DuplicateGroup, DuplicateMode, DuplicateScope,
    };

    let DuplicateFunctionsConfig {
        roots,
        elide_identifiers,
        elide_literals,
        show_source,
        min_lines,
        include_trait_impls,
        filter,
    } = cfg;
    let extractor = Extractor::new();
    let multi_repo = roots.len() > 1;

    let mut hash_groups: HashMap<u64, Vec<DuplicateFunctionLocation>> = HashMap::new();
    let mut files_scanned = 0usize;
    let mut functions_hashed = 0usize;
    let mut combined_allowlist: HashSet<String> = HashSet::new();

    for root in roots {
        let allowlist = load_duplicate_functions_allowlist(root);
        let repo_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

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
                if let Some(f) = filter {
                    let rel_path = e.path().strip_prefix(root).unwrap_or(e.path());
                    f.matches(rel_path)
                } else {
                    true
                }
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        // (fn_count, Vec<(hash, location)>)
        let per_file: Vec<(usize, Vec<(u64, DuplicateFunctionLocation)>)> = files
            .par_iter()
            .filter_map(|path| {
                let content = std::fs::read_to_string(path).ok()?;
                let support = support_for_path(path)?;
                let tree = parsers::parse_with_grammar(support.grammar_name(), &content)?;
                let result = extractor.extract(path, &content);
                let root_rel = path.strip_prefix(root).unwrap_or(path);
                let rel_path = if multi_repo {
                    format!("{}/{}", repo_name, root_rel.display())
                } else {
                    root_rel.display().to_string()
                };

                let mut entries = Vec::new();
                for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
                    let kind = sym.kind.as_str();
                    if kind != "function" && kind != "method" {
                        continue;
                    }
                    if let Some(node) = find_function_node(&tree, sym.start_line) {
                        let line_count = sym.end_line.saturating_sub(sym.start_line) + 1;
                        if line_count < min_lines {
                            continue;
                        }
                        let hash = compute_function_hash(
                            &node,
                            content.as_bytes(),
                            elide_identifiers,
                            elide_literals,
                        );
                        entries.push((
                            hash,
                            DuplicateFunctionLocation {
                                file: rel_path.clone(),
                                symbol: sym.name.clone(),
                                start_line: sym.start_line,
                                end_line: sym.end_line,
                            },
                        ));
                    }
                }
                Some((entries.len(), entries))
            })
            .collect();

        for (fn_count, entries) in per_file {
            files_scanned += 1;
            functions_hashed += fn_count;
            for (hash, loc) in entries {
                hash_groups.entry(hash).or_default().push(loc);
            }
        }

        for key in allowlist {
            if multi_repo {
                combined_allowlist.insert(format!("{}/{}", repo_name, key));
            } else {
                combined_allowlist.insert(key);
            }
        }
    }

    let mut groups: Vec<DuplicateFunctionGroup> = hash_groups
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .filter(|(_, locs)| {
            locs.iter()
                .any(|loc| !combined_allowlist.contains(&format!("{}:{}", loc.file, loc.symbol)))
        })
        .map(|(hash, locations)| {
            let line_count = locations
                .first()
                .map(|l| l.end_line - l.start_line + 1)
                .unwrap_or(0);
            DuplicateFunctionGroup {
                hash,
                locations,
                line_count,
            }
        })
        .collect();

    let suppressed_same_name = if include_trait_impls {
        0
    } else {
        let before = groups.len();
        groups.retain(|g| {
            let names: std::collections::HashSet<&str> =
                g.locations.iter().map(|l| l.symbol.as_str()).collect();
            names.len() > 1
        });
        before - groups.len()
    };

    groups.sort_by(|a, b| {
        b.line_count
            .cmp(&a.line_count)
            .then_with(|| b.locations.len().cmp(&a.locations.len()))
    });

    let duplicated_lines: usize = groups
        .iter()
        .map(|g| g.line_count * g.locations.len())
        .sum();

    let unified_groups: Vec<DuplicateGroup> = groups
        .into_iter()
        .map(|g| DuplicateGroup {
            locations: g
                .locations
                .into_iter()
                .map(|l| CodeLocation {
                    file: l.file,
                    symbol: Some(l.symbol),
                    start_line: l.start_line,
                    end_line: l.end_line,
                })
                .collect(),
            line_count: g.line_count,
            hash: Some(format!("{:016x}", g.hash)),
            similarity: None,
            pair_count: None,
        })
        .collect();

    DuplicatesReport {
        mode: DuplicateMode::Exact,
        scope: DuplicateScope::Functions,
        files_scanned,
        items_analyzed: functions_hashed,
        pairs_analyzed: None,
        threshold: None,
        elide_identifiers: Some(elide_identifiers),
        elide_literals: Some(elide_literals),
        duplicated_lines: Some(duplicated_lines),
        suppressed_same_name: Some(suppressed_same_name),
        stats: None,
        groups: unified_groups,
        show_source,
        roots: roots.to_vec(),
    }
}

/// Build duplicate blocks report without printing (for service layer).
pub fn build_duplicate_blocks_report(cfg: DuplicateBlocksConfig<'_>) -> DuplicatesReport {
    use crate::commands::analyze::duplicates_views::{
        CodeLocation, DuplicateGroup, DuplicateMode, DuplicateScope,
    };

    let DuplicateBlocksConfig {
        root,
        min_lines,
        elide_identifiers,
        elide_literals,
        skip_functions,
        show_source,
        allow: _allow,
        reason: _reason,
        filter,
    } = cfg;
    let extractor = Extractor::new();
    let mut hash_groups: HashMap<u64, Vec<DuplicateBlockLocation>> = HashMap::new();
    let mut file_extractions: HashMap<String, normalize_facts::extract::ExtractResult> =
        HashMap::new();
    let mut files_scanned = 0usize;
    let mut blocks_hashed = 0usize;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();

        if let Some(f) = filter {
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if !f.matches(rel_path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        let tree = match crate::parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        files_scanned += 1;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        file_extractions.insert(rel_path.clone(), extractor.extract(path, &content));

        let before = hash_groups.values().map(|v| v.len()).sum::<usize>();
        collect_block_hashes(
            &tree.root_node(),
            content.as_bytes(),
            &rel_path,
            min_lines,
            elide_identifiers,
            elide_literals,
            skip_functions,
            &mut hash_groups,
        );
        let after = hash_groups.values().map(|v| v.len()).sum::<usize>();
        blocks_hashed += after - before;
    }

    let groups_raw: Vec<DuplicateBlockGroup> = hash_groups
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .map(|(hash, locations)| {
            let line_count = locations
                .first()
                .map(|l| l.end_line.saturating_sub(l.start_line) + 1)
                .unwrap_or(0);
            DuplicateBlockGroup {
                hash,
                locations,
                line_count,
            }
        })
        .collect();

    let groups = suppress_contained_blocks(groups_raw);

    let allow_key = |loc: &DuplicateBlockLocation| {
        let func = file_extractions
            .get(&loc.file)
            .and_then(|r| containing_function(r, loc.start_line));
        block_allow_key(&loc.file, loc.start_line, loc.end_line, func.as_deref())
    };

    let allowlist = load_block_allowlist(root, "duplicate-blocks-allow");
    let loc_allowed = |loc: &DuplicateBlockLocation| {
        allowlist.contains(&allow_key(loc))
            || allowlist.contains(&format!("{}:{}-{}", loc.file, loc.start_line, loc.end_line))
    };
    let groups: Vec<DuplicateBlockGroup> = groups
        .into_iter()
        .filter(|g| !g.locations.iter().all(&loc_allowed))
        .collect();

    let unified_groups: Vec<DuplicateGroup> = groups
        .into_iter()
        .map(|g| DuplicateGroup {
            locations: g
                .locations
                .into_iter()
                .map(|l| CodeLocation {
                    file: l.file,
                    symbol: None,
                    start_line: l.start_line,
                    end_line: l.end_line,
                })
                .collect(),
            line_count: g.line_count,
            hash: Some(format!("{:016x}", g.hash)),
            similarity: None,
            pair_count: None,
        })
        .collect();

    DuplicatesReport {
        mode: DuplicateMode::Exact,
        scope: DuplicateScope::Blocks,
        files_scanned,
        items_analyzed: blocks_hashed,
        pairs_analyzed: None,
        threshold: None,
        elide_identifiers: Some(elide_identifiers),
        elide_literals: Some(elide_literals),
        duplicated_lines: None,
        suppressed_same_name: None,
        stats: None,
        groups: unified_groups,
        show_source,
        roots: vec![root.to_path_buf()],
    }
}

/// Build similar functions report without printing (for service layer).
pub fn build_similar_functions_report(cfg: SimilarFunctionsConfig<'_>) -> DuplicatesReport {
    use crate::commands::analyze::duplicates_views::{
        CodeLocation, DuplicateGroup, DuplicateMode, DuplicateScope,
    };

    let SimilarFunctionsConfig {
        roots,
        min_lines,
        similarity,
        elide_identifiers,
        elide_literals,
        skeleton,
        show_source,
        include_trait_impls,
        allow: _allow,
        reason: _reason,
        filter,
    } = cfg;

    let multi_repo = roots.len() > 1;
    let result = find_similar_function_pairs(
        roots,
        min_lines,
        similarity,
        elide_identifiers,
        elide_literals,
        skeleton,
        include_trait_impls,
        filter,
    );
    let files_scanned = result.files_scanned;
    let functions_analyzed = result.functions_analyzed;
    let pairs = result.pairs;

    let fn_allow_key = |file: &str, symbol: &str, start: usize, end: usize| {
        format!("{}:{}:{}-{}", file, symbol, start, end)
    };

    let combined_allowlist: HashSet<String> = roots
        .iter()
        .flat_map(|root| {
            let repo_name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            load_block_allowlist(root, "similar-functions-allow")
                .into_iter()
                .map(move |k| {
                    if multi_repo {
                        format!("{}/{}", repo_name, k)
                    } else {
                        k
                    }
                })
        })
        .collect();
    let pairs: Vec<SimilarFunctionPair> = pairs
        .into_iter()
        .filter(|p| {
            !(combined_allowlist.contains(&fn_allow_key(
                &p.file_a,
                &p.symbol_a,
                p.start_line_a,
                p.end_line_a,
            )) && combined_allowlist.contains(&fn_allow_key(
                &p.file_b,
                &p.symbol_b,
                p.start_line_b,
                p.end_line_b,
            )))
        })
        .collect();

    let unified_groups: Vec<DuplicateGroup> = pairs
        .into_iter()
        .map(|p| DuplicateGroup {
            locations: vec![
                CodeLocation {
                    file: p.file_a,
                    symbol: Some(p.symbol_a),
                    start_line: p.start_line_a,
                    end_line: p.end_line_a,
                },
                CodeLocation {
                    file: p.file_b,
                    symbol: Some(p.symbol_b),
                    start_line: p.start_line_b,
                    end_line: p.end_line_b,
                },
            ],
            line_count: p.line_count,
            hash: None,
            similarity: Some(p.similarity),
            pair_count: None,
        })
        .collect();

    DuplicatesReport {
        mode: DuplicateMode::Similar,
        scope: DuplicateScope::Functions,
        files_scanned,
        items_analyzed: functions_analyzed,
        pairs_analyzed: None,
        threshold: Some(similarity),
        elide_identifiers: None,
        elide_literals: None,
        duplicated_lines: None,
        suppressed_same_name: None,
        stats: None,
        groups: unified_groups,
        show_source,
        roots: roots.to_vec(),
    }
}

/// Build similar blocks report without printing (for service layer).
pub fn build_similar_blocks_report(cfg: SimilarBlocksConfig<'_>) -> DuplicatesReport {
    use crate::commands::analyze::duplicates_views::{
        CodeLocation, DuplicateGroup, DuplicateMode, DuplicateScope,
    };

    let SimilarBlocksConfig {
        root,
        min_lines,
        similarity,
        elide_identifiers,
        elide_literals,
        skeleton,
        show_source,
        include_trait_impls,
        allow: _allow,
        reason: _reason,
        filter,
    } = cfg;
    let extractor = Extractor::new();
    let mut all_blocks: Vec<(DuplicateBlockLocation, [u64; MINHASH_N])> = Vec::new();
    let mut file_extractions: HashMap<String, normalize_facts::extract::ExtractResult> =
        HashMap::new();
    let mut files_scanned = 0usize;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        let path = e.path();
        path.is_file() && super::is_source_file(path)
    }) {
        let path = entry.path();

        if let Some(f) = filter {
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if !f.matches(rel_path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let support = match support_for_path(path) {
            Some(s) => s,
            None => continue,
        };

        let tree = match crate::parsers::parse_with_grammar(support.grammar_name(), &content) {
            Some(t) => t,
            None => continue,
        };

        files_scanned += 1;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        file_extractions.insert(rel_path.clone(), extractor.extract(path, &content));

        collect_block_signatures(
            &tree.root_node(),
            content.as_bytes(),
            &rel_path,
            min_lines,
            elide_identifiers,
            elide_literals,
            skeleton,
            &mut all_blocks,
        );
    }

    let blocks_analyzed = all_blocks.len();

    let mut band_buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, (_, sig)) in all_blocks.iter().enumerate() {
        for band in 0..LSH_BANDS {
            let bh = lsh_band_hash(sig, band);
            let key = bh.wrapping_add((band as u64).wrapping_mul(0x9e3779b97f4a7c15));
            band_buckets.entry(key).or_default().push(idx);
        }
    }

    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for bucket in band_buckets.values() {
        if bucket.len() < 2 {
            continue;
        }
        for i in 0..bucket.len() {
            for j in i + 1..bucket.len() {
                let (a, b) = (bucket[i].min(bucket[j]), bucket[i].max(bucket[j]));
                if seen.insert((a, b)) {
                    candidates.push((a, b));
                }
            }
        }
    }

    let mut pairs: Vec<SimilarBlockPair> = candidates
        .into_iter()
        .filter_map(|(i, j)| {
            let (loc_a, sig_a) = &all_blocks[i];
            let (loc_b, sig_b) = &all_blocks[j];

            if loc_a.file == loc_b.file
                && loc_a.start_line == loc_b.start_line
                && loc_a.end_line == loc_b.end_line
            {
                return None;
            }

            if loc_a.file == loc_b.file {
                let a_contains_b =
                    loc_a.start_line <= loc_b.start_line && loc_b.end_line <= loc_a.end_line;
                let b_contains_a =
                    loc_b.start_line <= loc_a.start_line && loc_a.end_line <= loc_b.end_line;
                if a_contains_b || b_contains_a {
                    return None;
                }
            }

            {
                let len_a = loc_a.end_line.saturating_sub(loc_a.start_line) + 1;
                let len_b = loc_b.end_line.saturating_sub(loc_b.start_line) + 1;
                let ratio = len_a.min(len_b) as f64 / len_a.max(len_b) as f64;
                let min_ratio = if skeleton { 0.2 } else { 0.5 };
                if ratio < min_ratio {
                    return None;
                }
            }

            let sim = jaccard_estimate(sig_a, sig_b);
            if sim >= 1.0 || sim < similarity {
                return None;
            }

            let line_count = loc_a
                .end_line
                .saturating_sub(loc_a.start_line)
                .max(loc_b.end_line.saturating_sub(loc_b.start_line))
                + 1;

            Some(SimilarBlockPair {
                location_a: loc_a.clone(),
                location_b: loc_b.clone(),
                similarity: sim,
                line_count,
            })
        })
        .collect();

    pairs.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.line_count.cmp(&a.line_count))
    });

    let pairs = suppress_overlapping_pairs(pairs);

    let pairs = if include_trait_impls {
        pairs
    } else {
        pairs
            .into_iter()
            .filter(|p| {
                let fn_a = file_extractions
                    .get(&p.location_a.file)
                    .and_then(|r| containing_function(r, p.location_a.start_line));
                let fn_b = file_extractions
                    .get(&p.location_b.file)
                    .and_then(|r| containing_function(r, p.location_b.start_line));
                !(fn_a.is_some() && fn_a == fn_b)
            })
            .collect()
    };

    let pair_allow_key = |loc: &DuplicateBlockLocation| {
        let func = file_extractions
            .get(&loc.file)
            .and_then(|r| containing_function(r, loc.start_line));
        block_allow_key(&loc.file, loc.start_line, loc.end_line, func.as_deref())
    };

    let allowlist = load_block_allowlist(root, "similar-blocks-allow");
    let loc_in_allowlist = |loc: &DuplicateBlockLocation| {
        allowlist.contains(&pair_allow_key(loc))
            || allowlist.contains(&format!("{}:{}-{}", loc.file, loc.start_line, loc.end_line))
    };
    let pairs: Vec<SimilarBlockPair> = pairs
        .into_iter()
        .filter(|p| !(loc_in_allowlist(&p.location_a) && loc_in_allowlist(&p.location_b)))
        .collect();

    let unified_groups: Vec<DuplicateGroup> = pairs
        .into_iter()
        .map(|p| DuplicateGroup {
            locations: vec![
                CodeLocation {
                    file: p.location_a.file,
                    symbol: None,
                    start_line: p.location_a.start_line,
                    end_line: p.location_a.end_line,
                },
                CodeLocation {
                    file: p.location_b.file,
                    symbol: None,
                    start_line: p.location_b.start_line,
                    end_line: p.location_b.end_line,
                },
            ],
            line_count: p.line_count,
            hash: None,
            similarity: Some(p.similarity),
            pair_count: None,
        })
        .collect();

    DuplicatesReport {
        mode: DuplicateMode::Similar,
        scope: DuplicateScope::Blocks,
        files_scanned,
        items_analyzed: blocks_analyzed,
        pairs_analyzed: None,
        threshold: Some(similarity),
        elide_identifiers: None,
        elide_literals: None,
        duplicated_lines: None,
        suppressed_same_name: None,
        stats: None,
        groups: unified_groups,
        show_source,
        roots: vec![root.to_path_buf()],
    }
}

/// Build duplicate types report without printing (for service layer).
pub fn build_duplicate_types_report(
    root: &Path,
    config_root: &Path,
    min_overlap_percent: usize,
) -> DuplicateTypesReport {
    use regex::Regex;

    let extractor = Extractor::new();

    let allowlist_path = config_root.join(".normalize/duplicate-types-allow");
    let allowed_pairs: HashSet<(String, String)> = std::fs::read_to_string(&allowlist_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() == 2 {
                let (a, b) = if parts[0] < parts[1] {
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    (parts[1].to_string(), parts[0].to_string())
                };
                Some((a, b))
            } else {
                None
            }
        })
        .collect();

    let mut types: Vec<TypeInfo> = Vec::new();
    let mut files_scanned = 0;
    // normalize-syntax-allow: rust/unwrap-in-impl - compile-time constant regex pattern
    let field_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(\w+)\s*:\s*\S").unwrap();

    let files: Vec<PathBuf> = if root.is_file() {
        vec![root.to_path_buf()]
    } else {
        ignore::WalkBuilder::new(root)
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
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    for path in &files {
        let path = path.as_path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        files_scanned += 1;
        let result = extractor.extract(path, &content);
        let lines: Vec<&str> = content.lines().collect();
        for sym in result.symbols.iter().flat_map(|s| flatten_symbols(s)) {
            let kind = sym.kind.as_str();
            if !matches!(kind, "struct" | "class" | "interface" | "type") {
                continue;
            }
            let start = sym.start_line.saturating_sub(1);
            let end = sym.end_line.min(lines.len());
            let source: String = lines[start..end].join("\n");
            let fields: Vec<String> = field_re
                .captures_iter(&source)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .collect();
            if fields.len() < 2 {
                continue;
            }
            let rel_path = if root.is_file() {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
            } else {
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            };
            types.push(TypeInfo {
                file: rel_path,
                name: sym.name.clone(),
                start_line: sym.start_line,
                fields,
            });
        }
    }

    let n = types.len() as f64;
    let mut field_df: HashMap<&str, usize> = HashMap::new();
    for t in &types {
        for f in t.fields.iter() {
            *field_df.entry(f.as_str()).or_insert(0) += 1;
        }
    }
    let idf = |field: &str| -> f64 {
        let df = field_df.get(field).copied().unwrap_or(1) as f64;
        (1.0 + n / df).ln()
    };

    let mut duplicates: Vec<DuplicatePair> = Vec::new();
    for i in 0..types.len() {
        for j in (i + 1)..types.len() {
            let t1 = &types[i];
            let t2 = &types[j];
            if t1.name == t2.name {
                continue;
            }
            let pair_key = if t1.name < t2.name {
                (t1.name.clone(), t2.name.clone())
            } else {
                (t2.name.clone(), t1.name.clone())
            };
            if allowed_pairs.contains(&pair_key) {
                continue;
            }
            let set1: HashSet<_> = t1.fields.iter().collect();
            let set2: HashSet<_> = t2.fields.iter().collect();
            let common: Vec<String> = set1.intersection(&set2).map(|s| (*s).clone()).collect();
            if common.len() < 3 {
                continue;
            }
            let weighted_common: f64 = common.iter().map(|f| idf(f)).sum();
            let weighted_union: f64 = set1.union(&set2).map(|f| idf(f.as_str())).sum();
            let overlap_percent = if weighted_union > 0.0 {
                (weighted_common / weighted_union * 100.0) as usize
            } else {
                0
            };
            if overlap_percent >= min_overlap_percent {
                duplicates.push(DuplicatePair {
                    type1: t1.clone(),
                    type2: t2.clone(),
                    overlap_percent,
                    common_fields: common,
                });
            }
        }
    }
    duplicates.sort_by(|a, b| b.overlap_percent.cmp(&a.overlap_percent));

    DuplicateTypesReport {
        files_scanned,
        types_analyzed: types.len(),
        min_overlap_percent,
        duplicates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_duplicate_functions_allowlist_empty() {
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let tmp = tempdir().unwrap();
        let allowlist = load_duplicate_functions_allowlist(tmp.path());
        assert!(allowlist.is_empty());
    }

    #[test]
    fn test_suppress_contained_blocks_removes_children() {
        // Parent group: 20 lines, child group: 5 lines (contained within parent)
        let parent = DuplicateBlockGroup {
            hash: 1,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 1,
                    end_line: 20,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 1,
                    end_line: 20,
                },
            ],
            line_count: 20,
        };
        let child = DuplicateBlockGroup {
            hash: 2,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 5,
                    end_line: 10,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 5,
                    end_line: 10,
                },
            ],
            line_count: 6,
        };
        let result = suppress_contained_blocks(vec![parent, child]);
        assert_eq!(result.len(), 1, "child group should be suppressed");
        assert_eq!(result[0].line_count, 20);
    }

    #[test]
    fn test_suppress_contained_blocks_keeps_non_overlapping() {
        let a = DuplicateBlockGroup {
            hash: 1,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 1,
                    end_line: 10,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 1,
                    end_line: 10,
                },
            ],
            line_count: 10,
        };
        let b = DuplicateBlockGroup {
            hash: 2,
            locations: vec![
                DuplicateBlockLocation {
                    file: "a.rs".into(),
                    start_line: 20,
                    end_line: 30,
                },
                DuplicateBlockLocation {
                    file: "b.rs".into(),
                    start_line: 20,
                    end_line: 30,
                },
            ],
            line_count: 11,
        };
        let result = suppress_contained_blocks(vec![a, b]);
        assert_eq!(
            result.len(),
            2,
            "non-overlapping groups should both survive"
        );
    }
}
