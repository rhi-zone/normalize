//! Dependency depth map: per-module depth in the import stack + ripple risk.
//!
//! Depth 0 = entry point (nothing imports it). High depth = deeply embedded.
//! Ripple score = fan_out × depth × downstream — composite blast radius metric.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use normalize_architecture::{build_import_graph, compute_depth, compute_downstream};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One module's depth and risk metrics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DepthEntry {
    /// Relative file path
    pub module: String,
    /// Longest chain of transitive importers (0 = entry point)
    pub depth: usize,
    /// Direct importer count
    pub fan_in: usize,
    /// Direct import count (what this module imports)
    pub fan_out: usize,
    /// Transitive reverse-dependency count (BFS through importers)
    pub downstream: usize,
    /// Composite blast radius: fan_out × depth × downstream
    pub ripple_score: usize,
}

/// Summary statistics for the depth map.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DepthMapStats {
    pub total_modules: usize,
    pub max_depth: usize,
    pub avg_depth: f64,
    pub max_ripple_score: usize,
    pub modules_at_depth_0: usize,
}

/// Full depth-map report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DepthMapReport {
    pub modules: Vec<DepthEntry>,
    pub stats: DepthMapStats,
}

/// Analyze the dependency depth map.
pub async fn analyze_depth_map(
    idx: &FileIndex,
    limit: usize,
) -> Result<DepthMapReport, libsql::Error> {
    let graph = build_import_graph(idx).await?;

    // Collect all modules (files that appear in either side of the graph)
    let mut all_modules: HashSet<String> = HashSet::new();
    for (file, targets) in &graph.imports_by_file {
        all_modules.insert(file.clone());
        for t in targets {
            all_modules.insert(t.clone());
        }
    }
    for (file, importers) in &graph.importers_by_file {
        all_modules.insert(file.clone());
        for i in importers {
            all_modules.insert(i.clone());
        }
    }

    // Compute depth for every module
    let mut depth_memo: HashMap<String, usize> = HashMap::new();
    let mut in_stack: HashSet<String> = HashSet::new();
    for module in &all_modules {
        compute_depth(
            module,
            &graph.importers_by_file,
            &mut depth_memo,
            &mut in_stack,
        );
    }

    // Build entries
    let mut entries: Vec<DepthEntry> = all_modules
        .iter()
        .map(|module| {
            let depth = depth_memo.get(module).copied().unwrap_or(0);
            let fan_in = graph
                .importers_by_file
                .get(module)
                .map(|s| s.len())
                .unwrap_or(0);
            let fan_out = graph
                .imports_by_file
                .get(module)
                .map(|s| s.len())
                .unwrap_or(0);
            let downstream = compute_downstream(module, &graph.importers_by_file);
            let ripple_score = fan_out * depth * downstream;

            DepthEntry {
                module: module.clone(),
                depth,
                fan_in,
                fan_out,
                downstream,
                ripple_score,
            }
        })
        .collect();

    // Compute stats on full list before truncation
    let total_modules = entries.len();
    let max_depth = entries.iter().map(|e| e.depth).max().unwrap_or(0);
    let avg_depth = if total_modules > 0 {
        entries.iter().map(|e| e.depth).sum::<usize>() as f64 / total_modules as f64
    } else {
        0.0
    };
    let max_ripple_score = entries.iter().map(|e| e.ripple_score).max().unwrap_or(0);
    let modules_at_depth_0 = entries.iter().filter(|e| e.depth == 0).count();

    let stats = DepthMapStats {
        total_modules,
        max_depth,
        avg_depth,
        max_ripple_score,
        modules_at_depth_0,
    };

    normalize_analyze::ranked::rank_and_truncate(
        &mut entries,
        limit,
        |a, b| {
            b.ripple_score
                .cmp(&a.ripple_score)
                .then(b.depth.cmp(&a.depth))
                .then(a.module.cmp(&b.module))
        },
        |e| e.ripple_score as f64,
    );

    Ok(DepthMapReport {
        modules: entries,
        stats,
    })
}

/// CLI entry point.
pub fn analyze_depth_map_sync(root: &Path, limit: usize) -> Result<DepthMapReport, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create async runtime: {}", e))?;

    rt.block_on(async {
        let idx = crate::index::ensure_ready(root).await?;
        analyze_depth_map(&idx, limit)
            .await
            .map_err(|e| format!("Depth map analysis failed: {}", e))
    })
}

impl OutputFormatter for DepthMapReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "# Depth Map — {} modules, max depth {}, avg {:.1}, {} entry points",
            self.stats.total_modules,
            self.stats.max_depth,
            self.stats.avg_depth,
            self.stats.modules_at_depth_0,
        ));
        out.push(String::new());

        if self.modules.is_empty() {
            out.push("No import data found. Run `normalize facts rebuild` first.".to_string());
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
            "{:<width$}  {:>5}  {:>6}  {:>7}  {:>10}  {:>12}",
            "Module",
            "Depth",
            "Fan-in",
            "Fan-out",
            "Downstream",
            "Ripple Score",
            width = max_mod_len
        ));
        out.push(format!(
            "{}--------------------------------------------------",
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
                "{:<width$}  {:>5}  {:>6}  {:>7}  {:>10}  {:>12}",
                module,
                entry.depth,
                entry.fan_in,
                entry.fan_out,
                entry.downstream,
                entry.ripple_score,
                width = max_mod_len
            ));
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "\x1b[1;36m# Depth Map\x1b[0m — \x1b[1m{}\x1b[0m modules, max depth \x1b[1;33m{}\x1b[0m, avg \x1b[33m{:.1}\x1b[0m, \x1b[32m{}\x1b[0m entry points",
            self.stats.total_modules,
            self.stats.max_depth,
            self.stats.avg_depth,
            self.stats.modules_at_depth_0,
        ));
        out.push(String::new());

        if self.modules.is_empty() {
            out.push("No import data found. Run `normalize facts rebuild` first.".to_string());
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
            "\x1b[1m{:<width$}  {:>5}  {:>6}  {:>7}  {:>10}  {:>12}\x1b[0m",
            "Module",
            "Depth",
            "Fan-in",
            "Fan-out",
            "Downstream",
            "Ripple Score",
            width = max_mod_len
        ));
        out.push(format!(
            "{}--------------------------------------------------",
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

            let depth_color = if entry.depth >= 5 {
                "\x1b[1;31m"
            } else if entry.depth >= 3 {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };

            let ripple_color = if entry.ripple_score > 100 {
                "\x1b[1;31m"
            } else if entry.ripple_score > 20 {
                "\x1b[33m"
            } else {
                "\x1b[0m"
            };

            out.push(format!(
                "{:<width$}  {}{:>5}\x1b[0m  {:>6}  {:>7}  {:>10}  {}{:>12}\x1b[0m",
                module,
                depth_color,
                entry.depth,
                entry.fan_in,
                entry.fan_out,
                entry.downstream,
                ripple_color,
                entry.ripple_score,
                width = max_mod_len
            ));
        }

        out.join("\n")
    }
}
