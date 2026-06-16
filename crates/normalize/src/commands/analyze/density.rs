use flate2::{Compression, write::GzEncoder};
use normalize_analyze::ranked::{
    Column, DiffableRankEntry, RankEntry, Scored, format_delta, format_ranked_table, rank_pipeline,
};
use normalize_languages::support_for_path;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
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
    /// combined: (compression_ratio + token_uniqueness) / 2
    pub density_score: f64,
    pub lines: usize,
}

/// Per-module density summary.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ModuleDensity {
    pub module: String,
    pub avg_compression_ratio: f64,
    pub avg_token_uniqueness: f64,
    /// combined: (avg_compression_ratio + avg_token_uniqueness) / 2
    pub density_score: f64,
    pub total_files: usize,
    pub total_lines: usize,
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl normalize_analyze::Entity for FileDensity {
    fn label(&self) -> &str {
        &self.path
    }
}

impl normalize_analyze::Entity for ModuleDensity {
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
            Column::right("Lines"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            format!("{:.3}", self.density_score),
            self.path.clone(),
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
            "# {}Code Density — {}, {} files, compression {:.3}, uniqueness {:.3}",
            diff_prefix,
            self.root,
            self.files_analyzed,
            self.overall_compression_ratio,
            self.overall_token_uniqueness,
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
            "# {}Code Density — {}, {} files, compression {:.3}, uniqueness {:.3}",
            diff_prefix,
            self.root,
            self.files_analyzed,
            self.overall_compression_ratio,
            self.overall_token_uniqueness,
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

/// Analyze information density across the codebase.
pub fn analyze_density(root: &Path, module_limit: usize, worst_limit: usize) -> DensityReport {
    let module_dirs = discover_module_dirs(root);
    let all_files = crate::path_resolve::all_files(root);

    let file_metrics: Vec<FileDensity> = all_files
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
            let density_score = (comp_ratio + tok_uniq) / 2.0;
            let lines = content.lines().count();
            Some(FileDensity {
                path: f.path.clone(),
                total_bytes,
                compressed_bytes: (comp_ratio * total_bytes as f64) as usize,
                compression_ratio: comp_ratio,
                total_tokens: tok.total,
                unique_tokens: tok.unique,
                token_uniqueness: tok_uniq,
                density_score,
                lines,
            })
        })
        .collect();

    // Per-module aggregation
    let mut module_data: BTreeMap<String, (Vec<f64>, Vec<f64>, usize, usize)> = BTreeMap::new();
    for fd in &file_metrics {
        let key = module_key(&fd.path, &module_dirs);
        let entry = module_data.entry(key).or_default();
        entry.0.push(fd.compression_ratio);
        entry.1.push(fd.token_uniqueness);
        entry.2 += 1;
        entry.3 += fd.lines;
    }

    let modules: Vec<ModuleDensity> = module_data
        .into_iter()
        .map(
            |(module, (comp_ratios, tok_uniqs, total_files, total_lines))| {
                let avg_comp = comp_ratios.iter().sum::<f64>() / comp_ratios.len() as f64;
                let avg_tok = tok_uniqs.iter().sum::<f64>() / tok_uniqs.len() as f64;
                let density_score = (avg_comp + avg_tok) / 2.0;
                ModuleDensity {
                    module,
                    avg_compression_ratio: avg_comp,
                    avg_token_uniqueness: avg_tok,
                    density_score,
                    total_files,
                    total_lines,
                    delta: None,
                }
            },
        )
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
    let (overall_comp, overall_tok) = if file_metrics.is_empty() {
        (1.0, 1.0)
    } else {
        let n = files_analyzed as f64;
        let c = file_metrics
            .iter()
            .map(|f| f.compression_ratio)
            .sum::<f64>()
            / n;
        let t = file_metrics.iter().map(|f| f.token_uniqueness).sum::<f64>() / n;
        (c, t)
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
        modules,
        worst_files,
        diff_ref: None,
    }
}
