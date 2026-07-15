use flate2::{Compression, write::GzEncoder};
use normalize_facts_core::split_identifier_words;
use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;
use normalize_rank::ranked::{
    Column, DiffableRankEntry, RankEntry, Scored, format_delta, format_ranked_table, rank_pipeline,
};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use crate::commands::analyze::test_ratio::{discover_module_dirs, module_key};
use crate::output::OutputFormatter;

/// Per-file density metrics.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FileDensity {
    pub path: String,
    pub total_bytes: usize,
    pub compressed_bytes: usize,
    /// compressed / total — lower = more repetitive, higher = denser
    pub compression_ratio: f64,
    pub total_tokens: usize,
    pub unique_tokens: usize,
    /// unique / total — lower = more repetitive
    pub token_uniqueness: f64,
    /// Shannon entropy over AST node-type kinds, normalized to \[0, 1\].
    pub structural_entropy: f64,
    /// Shannon entropy over identifier word fragments, normalized to \[0, 1\].
    pub vocabulary_entropy: f64,
    /// KL divergence of this file's vocabulary from the project-wide vocabulary.
    pub cross_file_entropy: f64,
    /// combined: average of compression_ratio, token_uniqueness, structural_entropy,
    /// vocabulary_entropy, cross_file_entropy
    pub density_score: f64,
    pub lines: usize,
}

/// Per-module density summary.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ModuleDensity {
    pub module: String,
    pub avg_compression_ratio: f64,
    pub avg_token_uniqueness: f64,
    pub avg_structural_entropy: f64,
    pub avg_vocabulary_entropy: f64,
    pub avg_cross_file_entropy: f64,
    /// combined: average of the five per-file metrics
    pub density_score: f64,
    pub total_files: usize,
    pub total_lines: usize,
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl normalize_rank::Entity for FileDensity {
    fn label(&self) -> &str {
        &self.path
    }
}

impl normalize_rank::Entity for ModuleDensity {
    fn label(&self) -> &str {
        &self.module
    }
}

impl RankEntry for ModuleDensity {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Module"),
            Column::right("Files"),
            Column::right("Compression"),
            Column::right("Unique"),
            Column::right("Structural"),
            Column::right("Vocab"),
            Column::right("CrossFile"),
            Column::right("Density"),
            Column::right("Lines"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let density_str = match self.delta {
            Some(d) => format!("{:.3} ({})", self.density_score, format_delta(d, false)),
            None => format!("{:.3}", self.density_score),
        };
        vec![
            self.module.clone(),
            self.total_files.to_string(),
            format!("{:.3}", self.avg_compression_ratio),
            format!("{:.3}", self.avg_token_uniqueness),
            format!("{:.3}", self.avg_structural_entropy),
            format!("{:.3}", self.avg_vocabulary_entropy),
            format!("{:.3}", self.avg_cross_file_entropy),
            density_str,
            self.total_lines.to_string(),
        ]
    }
}

impl DiffableRankEntry for ModuleDensity {
    fn diff_key(&self) -> &str {
        &self.module
    }
    fn diff_score(&self) -> f64 {
        self.density_score
    }
    fn set_delta(&mut self, delta: Option<f64>) {
        self.delta = delta;
    }
    fn delta(&self) -> Option<f64> {
        self.delta
    }
}

impl RankEntry for FileDensity {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Density"),
            Column::left("File"),
            Column::right("Structural"),
            Column::right("Vocab"),
            Column::right("CrossFile"),
            Column::right("Lines"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            format!("{:.3}", self.density_score),
            self.path.clone(),
            format!("{:.3}", self.structural_entropy),
            format!("{:.3}", self.vocabulary_entropy),
            format!("{:.3}", self.cross_file_entropy),
            self.lines.to_string(),
        ]
    }
}

/// Report returned by `analyze density`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DensityReport {
    pub root: String,
    pub files_analyzed: usize,
    pub overall_compression_ratio: f64,
    pub overall_token_uniqueness: f64,
    pub overall_structural_entropy: f64,
    pub overall_vocabulary_entropy: f64,
    pub overall_cross_file_entropy: f64,
    /// Modules sorted by density_score ascending (most repetitive first).
    pub modules: Vec<ModuleDensity>,
    /// Bottom N files by density_score (most repetitive first).
    pub worst_files: Vec<FileDensity>,
    /// Set when `--diff` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_ref: Option<String>,
}

impl OutputFormatter for DensityReport {
    fn format_text(&self) -> String {
        let diff_prefix = self
            .diff_ref
            .as_ref()
            .map_or(String::new(), |r| format!("Diff vs {} — ", r));
        let title = format!(
            "# {}Code Density — {}, {} files, compression {:.3}, uniqueness {:.3}, structural {:.3}, vocab {:.3}, cross-file {:.3}",
            diff_prefix,
            self.root,
            self.files_analyzed,
            self.overall_compression_ratio,
            self.overall_token_uniqueness,
            self.overall_structural_entropy,
            self.overall_vocabulary_entropy,
            self.overall_cross_file_entropy,
        );

        let mut out = format_ranked_table(&title, &self.modules, Some("No modules found."));

        if !self.worst_files.is_empty() {
            out.push_str("\n\n");
            out.push_str(&format_ranked_table(
                "## Most Repetitive Files",
                &self.worst_files,
                None,
            ));
        }

        out
    }

    fn format_pretty(&self) -> String {
        let diff_prefix = self
            .diff_ref
            .as_ref()
            .map_or(String::new(), |r| format!("Diff vs {} — ", r));
        let title = format!(
            "# {}Code Density — {}, {} files, compression {:.3}, uniqueness {:.3}, structural {:.3}, vocab {:.3}, cross-file {:.3}",
            diff_prefix,
            self.root,
            self.files_analyzed,
            self.overall_compression_ratio,
            self.overall_token_uniqueness,
            self.overall_structural_entropy,
            self.overall_vocabulary_entropy,
            self.overall_cross_file_entropy,
        );

        let mut out = crate::output::pretty_ranked_table(
            &title,
            &self.modules,
            Some("No modules found."),
            |_| None,
        );

        if !self.worst_files.is_empty() {
            out.push_str("\n\n");
            out.push_str(&crate::output::pretty_ranked_table(
                "## Most Repetitive Files",
                &self.worst_files,
                None,
                |_| None,
            ));
        }

        out
    }
}

/// Compress content with gzip and return compressed_len / original_len.
/// Returns 1.0 for empty content. Files smaller than min_bytes get None
/// to avoid gzip-overhead skew.
fn compression_ratio(content: &[u8]) -> Option<f64> {
    if content.is_empty() {
        return Some(1.0);
    }
    // Skip tiny files — gzip overhead dominates, ratios are misleading
    if content.len() < 200 {
        return None;
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content).unwrap_or(());
    let compressed = encoder.finish().unwrap_or_default();
    Some(compressed.len() as f64 / content.len() as f64)
}

struct TokenUniqueness {
    total: usize,
    unique: usize,
}

/// Split content into word-like tokens (alphanumeric + underscore, length > 1).
fn token_uniqueness(content: &str) -> TokenUniqueness {
    let tokens: Vec<&str> = content
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() > 1)
        .collect();
    if tokens.is_empty() {
        return TokenUniqueness {
            total: 0,
            unique: 0,
        };
    }
    let unique: HashSet<_> = tokens.iter().copied().collect();
    TokenUniqueness {
        total: tokens.len(),
        unique: unique.len(),
    }
}

/// Shannon entropy over a frequency distribution, normalized to \[0, 1\] by
/// dividing by log2(distinct count). Returns 0.0 when there are fewer than
/// two distinct keys (entropy is degenerate/undefined in that case).
fn shannon_entropy_normalized<K>(counts: &HashMap<K, usize>) -> f64 {
    let distinct = counts.len();
    if distinct < 2 {
        return 0.0;
    }
    let total: usize = counts.values().sum();
    if total == 0 {
        return 0.0;
    }
    let total_f = total as f64;
    let entropy: f64 = -counts
        .values()
        .map(|&c| {
            let p = c as f64 / total_f;
            p * p.log2()
        })
        .sum::<f64>();
    let max_entropy = (distinct as f64).log2();
    if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    }
}

/// Walk a tree-sitter tree and collect a frequency map of node-kind strings.
fn collect_node_kinds(tree: &tree_sitter::Tree) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut cursor = tree.walk();
    loop {
        *counts.entry(cursor.node().kind().to_string()).or_insert(0) += 1;
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return counts;
            }
        }
    }
}

/// Shannon entropy over the distribution of AST node-type kinds in a file,
/// normalized to \[0, 1\]. Returns 0.0 when the file has no grammar support
/// or fails to parse — genuinely unknown, not fabricated.
fn structural_entropy(content: &str, path: &Path) -> f64 {
    let Some(support) = support_for_path(path) else {
        return 0.0;
    };
    let Some(tree) = parse_with_grammar(support.grammar_name(), content) else {
        return 0.0;
    };
    let kinds = collect_node_kinds(&tree);
    shannon_entropy_normalized(&kinds)
}

/// Result of [`vocabulary_entropy`]: the normalized entropy plus the raw word
/// frequency map (the latter is needed for cross-file aggregation).
struct VocabularyEntropy {
    entropy: f64,
    word_counts: HashMap<String, usize>,
}

/// Shannon entropy over identifier word fragments in a file, normalized to
/// \[0, 1\], plus the raw word frequency map (for cross-file aggregation).
fn vocabulary_entropy(content: &str) -> VocabularyEntropy {
    let mut words: HashMap<String, usize> = HashMap::new();
    for token in content
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() > 1)
    {
        for word in split_identifier_words(token) {
            *words.entry(word).or_insert(0) += 1;
        }
    }
    let entropy = shannon_entropy_normalized(&words);
    VocabularyEntropy {
        entropy,
        word_counts: words,
    }
}

/// KL divergence of a file's word distribution from the project-wide word
/// distribution, with additive smoothing on the project distribution to
/// avoid division by zero for words unseen at the project level.
fn cross_file_entropy(
    file_words: &HashMap<String, usize>,
    project_words: &HashMap<String, usize>,
) -> f64 {
    let file_total: usize = file_words.values().sum();
    let project_total: usize = project_words.values().sum();
    if file_total == 0 || project_total == 0 {
        return 0.0;
    }
    let alpha = 1.0;
    let smoothed_total = project_total as f64 + alpha * project_words.len() as f64;
    let mut kl = 0.0;
    for (word, &count) in file_words {
        let p = count as f64 / file_total as f64;
        let q_count = project_words.get(word).copied().unwrap_or(0) as f64;
        let q = (q_count + alpha) / smoothed_total;
        if p > 0.0 && q > 0.0 {
            kl += p * (p / q).log2();
        }
    }
    kl
}

/// Intermediate per-file metrics before cross-file entropy is known — the
/// project-wide word distribution can only be computed after every file's
/// vocabulary has been collected in the first pass.
struct FileMetricsPass1 {
    path: String,
    total_bytes: usize,
    compression_ratio: f64,
    total_tokens: usize,
    unique_tokens: usize,
    token_uniqueness: f64,
    structural_entropy: f64,
    vocabulary_entropy: f64,
    word_counts: HashMap<String, usize>,
    lines: usize,
}

/// Analyze information density across the codebase.
pub fn analyze_density(root: &Path, module_limit: usize, worst_limit: usize) -> DensityReport {
    let module_dirs = discover_module_dirs(root);
    let all_files = crate::path_resolve::all_files(root);

    // Pass 1: per-file metrics that don't depend on the rest of the project,
    // including each file's word distribution (needed for pass 2).
    let pass1: Vec<FileMetricsPass1> = all_files
        .par_iter()
        .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
        .filter_map(|f| {
            let abs_path = root.join(&f.path);
            support_for_path(&abs_path)?;
            let content = std::fs::read_to_string(&abs_path).ok()?;
            if content.is_empty() {
                return None;
            }
            let bytes = content.as_bytes();
            let total_bytes = bytes.len();
            let comp_ratio = compression_ratio(bytes)?;
            let tok = token_uniqueness(&content);
            if tok.total == 0 {
                return None;
            }
            let tok_uniq = tok.unique as f64 / tok.total as f64;
            let struct_entropy = structural_entropy(&content, &abs_path);
            let vocab = vocabulary_entropy(&content);
            let (vocab_entropy, word_counts) = (vocab.entropy, vocab.word_counts);
            let lines = content.lines().count();
            Some(FileMetricsPass1 {
                path: f.path.clone(),
                total_bytes,
                compression_ratio: comp_ratio,
                total_tokens: tok.total,
                unique_tokens: tok.unique,
                token_uniqueness: tok_uniq,
                structural_entropy: struct_entropy,
                vocabulary_entropy: vocab_entropy,
                word_counts,
                lines,
            })
        })
        .collect();

    // Aggregate the project-wide word distribution from every file's vocabulary.
    let mut project_words: HashMap<String, usize> = HashMap::new();
    for fm in &pass1 {
        for (word, count) in &fm.word_counts {
            *project_words.entry(word.clone()).or_insert(0) += count;
        }
    }

    // Pass 2: cross-file entropy needs the project-wide distribution, then
    // assemble the final FileDensity records.
    let file_metrics: Vec<FileDensity> = pass1
        .into_par_iter()
        .map(|fm| {
            let cross_entropy = cross_file_entropy(&fm.word_counts, &project_words);
            let density_score = (fm.compression_ratio
                + fm.token_uniqueness
                + fm.structural_entropy
                + fm.vocabulary_entropy
                + cross_entropy)
                / 5.0;
            FileDensity {
                path: fm.path,
                total_bytes: fm.total_bytes,
                compressed_bytes: (fm.compression_ratio * fm.total_bytes as f64) as usize,
                compression_ratio: fm.compression_ratio,
                total_tokens: fm.total_tokens,
                unique_tokens: fm.unique_tokens,
                token_uniqueness: fm.token_uniqueness,
                structural_entropy: fm.structural_entropy,
                vocabulary_entropy: fm.vocabulary_entropy,
                cross_file_entropy: cross_entropy,
                density_score,
                lines: fm.lines,
            }
        })
        .collect();

    // Per-module aggregation
    #[derive(Default)]
    struct ModuleAcc {
        comp_ratios: Vec<f64>,
        tok_uniqs: Vec<f64>,
        struct_entropies: Vec<f64>,
        vocab_entropies: Vec<f64>,
        cross_entropies: Vec<f64>,
        total_files: usize,
        total_lines: usize,
    }
    let mut module_data: BTreeMap<String, ModuleAcc> = BTreeMap::new();
    for fd in &file_metrics {
        let key = module_key(&fd.path, &module_dirs);
        let entry = module_data.entry(key).or_default();
        entry.comp_ratios.push(fd.compression_ratio);
        entry.tok_uniqs.push(fd.token_uniqueness);
        entry.struct_entropies.push(fd.structural_entropy);
        entry.vocab_entropies.push(fd.vocabulary_entropy);
        entry.cross_entropies.push(fd.cross_file_entropy);
        entry.total_files += 1;
        entry.total_lines += fd.lines;
    }

    let modules: Vec<ModuleDensity> = module_data
        .into_iter()
        .map(|(module, acc)| {
            let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
            let avg_comp = avg(&acc.comp_ratios);
            let avg_tok = avg(&acc.tok_uniqs);
            let avg_struct = avg(&acc.struct_entropies);
            let avg_vocab = avg(&acc.vocab_entropies);
            let avg_cross = avg(&acc.cross_entropies);
            let density_score = (avg_comp + avg_tok + avg_struct + avg_vocab + avg_cross) / 5.0;
            ModuleDensity {
                module,
                avg_compression_ratio: avg_comp,
                avg_token_uniqueness: avg_tok,
                avg_structural_entropy: avg_struct,
                avg_vocabulary_entropy: avg_vocab,
                avg_cross_file_entropy: avg_cross,
                density_score,
                total_files: acc.total_files,
                total_lines: acc.total_lines,
                delta: None,
            }
        })
        .collect();

    // Use rank_pipeline: sort ascending (most repetitive = lowest score first) + truncate
    let mut scored_modules: Vec<Scored<_>> = modules
        .into_iter()
        .map(|m| {
            let score = m.density_score;
            Scored::new(m, score)
        })
        .collect();
    rank_pipeline(&mut scored_modules, module_limit, true);
    let modules: Vec<ModuleDensity> = scored_modules.into_iter().map(|s| s.entity).collect();

    // Overall aggregates (computed before file_metrics is consumed)
    let files_analyzed = file_metrics.len();
    let (overall_comp, overall_tok, overall_struct, overall_vocab, overall_cross) =
        if file_metrics.is_empty() {
            (1.0, 1.0, 0.0, 0.0, 0.0)
        } else {
            let n = files_analyzed as f64;
            let c = file_metrics
                .iter()
                .map(|f| f.compression_ratio)
                .sum::<f64>()
                / n;
            let t = file_metrics.iter().map(|f| f.token_uniqueness).sum::<f64>() / n;
            let s = file_metrics
                .iter()
                .map(|f| f.structural_entropy)
                .sum::<f64>()
                / n;
            let v = file_metrics
                .iter()
                .map(|f| f.vocabulary_entropy)
                .sum::<f64>()
                / n;
            let x = file_metrics
                .iter()
                .map(|f| f.cross_file_entropy)
                .sum::<f64>()
                / n;
            (c, t, s, v, x)
        };

    // Worst files by density_score (ascending) via rank_pipeline
    let mut scored_files: Vec<Scored<_>> = file_metrics
        .into_iter()
        .map(|f| {
            let score = f.density_score;
            Scored::new(f, score)
        })
        .collect();
    rank_pipeline(&mut scored_files, worst_limit, true);
    let worst_files: Vec<FileDensity> = scored_files.into_iter().map(|s| s.entity).collect();

    DensityReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        files_analyzed,
        overall_compression_ratio: overall_comp,
        overall_token_uniqueness: overall_tok,
        overall_structural_entropy: overall_struct,
        overall_vocabulary_entropy: overall_vocab,
        overall_cross_file_entropy: overall_cross,
        modules,
        worst_files,
        diff_ref: None,
    }
}
