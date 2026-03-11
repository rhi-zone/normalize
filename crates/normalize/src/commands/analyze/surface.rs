//! Interface surface area: per-module public symbol count, public ratio, and constraint score.
//!
//! Constraint score = public_symbols × fan_in — modules with wide interfaces relied on
//! by many dependents are the hardest to change safely.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::HashMap;

/// One module's interface metrics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SurfaceEntry {
    /// Relative file path
    pub module: String,
    /// Total symbol count (all visibilities)
    pub total_symbols: usize,
    /// Public symbol count
    pub public_symbols: usize,
    /// Non-public symbol count (private, protected, internal)
    pub private_symbols: usize,
    /// Fraction of symbols that are public (0.0–1.0)
    pub public_ratio: f64,
    /// Direct importer count (how many files import this module)
    pub fan_in: usize,
    /// public_symbols × fan_in — composite constraint metric
    pub constraint_score: usize,
}

/// Summary statistics for the surface report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SurfaceStats {
    /// Total files analyzed
    pub total_files: usize,
    /// Average public ratio across all files
    pub avg_public_ratio: f64,
    /// Highest constraint score
    pub max_constraint_score: usize,
    /// Files where every symbol is public
    pub fully_public_count: usize,
}

/// Full surface area report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SurfaceReport {
    pub modules: Vec<SurfaceEntry>,
    pub stats: SurfaceStats,
}

/// Analyze interface surface area.
pub async fn analyze_surface(
    idx: &FileIndex,
    limit: usize,
) -> Result<SurfaceReport, libsql::Error> {
    use super::architecture::build_import_graph;

    // 1. Load all symbols and group by file
    let symbols = idx.all_symbols_with_details().await?;

    let mut total_by_file: HashMap<String, usize> = HashMap::new();
    let mut public_by_file: HashMap<String, usize> = HashMap::new();

    for (file, _name, _kind, _start, _end, _parent, visibility, _is_impl) in &symbols {
        *total_by_file.entry(file.clone()).or_default() += 1;
        if visibility == "public" {
            *public_by_file.entry(file.clone()).or_default() += 1;
        }
    }

    // 2. Build import graph for fan-in
    let graph = build_import_graph(idx).await?;

    // 3. Build entries
    let mut entries: Vec<SurfaceEntry> = total_by_file
        .iter()
        .map(|(file, &total)| {
            let public = public_by_file.get(file).copied().unwrap_or(0);
            let private = total - public;
            let public_ratio = if total > 0 {
                public as f64 / total as f64
            } else {
                0.0
            };
            let fan_in = graph
                .importers_by_file
                .get(file)
                .map(|s| s.len())
                .unwrap_or(0);
            let constraint_score = public * fan_in;

            SurfaceEntry {
                module: file.clone(),
                total_symbols: total,
                public_symbols: public,
                private_symbols: private,
                public_ratio,
                fan_in,
                constraint_score,
            }
        })
        .collect();

    // 4. Compute stats before truncation
    let total_files = entries.len();
    let avg_public_ratio = if total_files > 0 {
        entries.iter().map(|e| e.public_ratio).sum::<f64>() / total_files as f64
    } else {
        0.0
    };
    let max_constraint_score = entries
        .iter()
        .map(|e| e.constraint_score)
        .max()
        .unwrap_or(0);
    let fully_public_count = entries
        .iter()
        .filter(|e| e.total_symbols > 0 && e.public_symbols == e.total_symbols)
        .count();

    let stats = SurfaceStats {
        total_files,
        avg_public_ratio,
        max_constraint_score,
        fully_public_count,
    };

    normalize_analyze::ranked::rank_and_truncate(
        &mut entries,
        limit,
        |a, b| {
            b.constraint_score
                .cmp(&a.constraint_score)
                .then(b.public_symbols.cmp(&a.public_symbols))
                .then(a.module.cmp(&b.module))
        },
        |e| e.constraint_score as f64,
    );

    Ok(SurfaceReport {
        modules: entries,
        stats,
    })
}

impl OutputFormatter for SurfaceReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "# Surface Area — {} files, avg public ratio {:.0}%, {} fully public, max constraint {}",
            self.stats.total_files,
            self.stats.avg_public_ratio * 100.0,
            self.stats.fully_public_count,
            self.stats.max_constraint_score,
        ));
        out.push(String::new());

        if self.modules.is_empty() {
            out.push("No symbol data found. Run `normalize structure rebuild` first.".to_string());
            return out.join("\n");
        }

        let max_mod_len = self
            .modules
            .iter()
            .map(|e| e.module.len())
            .max()
            .unwrap_or(10)
            .min(50);

        out.push(format!(
            "{:<width$}  {:>5}  {:>6}  {:>7}  {:>6}  {:>6}  {:>10}",
            "Module",
            "Total",
            "Public",
            "Private",
            "Ratio",
            "Fan-in",
            "Constraint",
            width = max_mod_len
        ));
        out.push(format!(
            "{}----------------------------------------------",
            "-".repeat(max_mod_len),
        ));

        for entry in &self.modules {
            let module = if entry.module.len() > max_mod_len {
                format!(
                    "...{}",
                    &entry.module[entry.module.len() - (max_mod_len - 3)..]
                )
            } else {
                entry.module.clone()
            };
            out.push(format!(
                "{:<width$}  {:>5}  {:>6}  {:>7}  {:>5.0}%  {:>6}  {:>10}",
                module,
                entry.total_symbols,
                entry.public_symbols,
                entry.private_symbols,
                entry.public_ratio * 100.0,
                entry.fan_in,
                entry.constraint_score,
                width = max_mod_len
            ));
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "\x1b[1;36m# Surface Area\x1b[0m — \x1b[1m{}\x1b[0m files, avg public ratio \x1b[33m{:.0}%\x1b[0m, \x1b[32m{}\x1b[0m fully public, max constraint \x1b[1;33m{}\x1b[0m",
            self.stats.total_files,
            self.stats.avg_public_ratio * 100.0,
            self.stats.fully_public_count,
            self.stats.max_constraint_score,
        ));
        out.push(String::new());

        if self.modules.is_empty() {
            out.push("No symbol data found. Run `normalize structure rebuild` first.".to_string());
            return out.join("\n");
        }

        let max_mod_len = self
            .modules
            .iter()
            .map(|e| e.module.len())
            .max()
            .unwrap_or(10)
            .min(50);

        out.push(format!(
            "\x1b[1m{:<width$}  {:>5}  {:>6}  {:>7}  {:>6}  {:>6}  {:>10}\x1b[0m",
            "Module",
            "Total",
            "Public",
            "Private",
            "Ratio",
            "Fan-in",
            "Constraint",
            width = max_mod_len
        ));
        out.push(format!(
            "{}----------------------------------------------",
            "-".repeat(max_mod_len),
        ));

        for entry in &self.modules {
            let module = if entry.module.len() > max_mod_len {
                format!(
                    "...{}",
                    &entry.module[entry.module.len() - (max_mod_len - 3)..]
                )
            } else {
                entry.module.clone()
            };

            let ratio_color = if entry.public_ratio >= 0.9 {
                "\x1b[1;31m"
            } else if entry.public_ratio >= 0.7 {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };

            let constraint_color = if entry.constraint_score > 100 {
                "\x1b[1;31m"
            } else if entry.constraint_score > 20 {
                "\x1b[33m"
            } else {
                "\x1b[0m"
            };

            out.push(format!(
                "{:<width$}  {:>5}  {:>6}  {:>7}  {}{:>5.0}%\x1b[0m  {:>6}  {}{:>10}\x1b[0m",
                module,
                entry.total_symbols,
                entry.public_symbols,
                entry.private_symbols,
                ratio_color,
                entry.public_ratio * 100.0,
                entry.fan_in,
                constraint_color,
                entry.constraint_score,
                width = max_mod_len
            ));
        }

        out.join("\n")
    }
}
