//! Import layering analysis: are imports flowing downward (good) or upward (coupling)?
//!
//! Uses dependency depth as the layer proxy — no hardcoded ordering.
//! Compliance = downward / (downward + upward), where 1.0 = perfect layering.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};
use normalize_architecture::{
    build_import_graph, compute_depth, compute_layering_compliance, extract_layer,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Per-module layering metrics.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct LayeringEntry {
    /// Relative file path
    pub module: String,
    /// Layer (first directory component)
    pub layer: String,
    /// Cross-layer imports (outgoing)
    pub total_imports: usize,
    /// Imports to deeper layers (good)
    pub downward_imports: usize,
    /// Imports to shallower layers (bad — coupling)
    pub upward_imports: usize,
    /// Imports within the same layer
    pub self_imports: usize,
    /// downward / (downward + upward); 1.0 if no cross-layer imports
    pub compliance: f64,
}

impl RankEntry for LayeringEntry {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Module"),
            Column::left("Layer"),
            Column::right("Cross"),
            Column::right("Down"),
            Column::right("Up"),
            Column::right("Self"),
            Column::right("Compliance"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            self.module.clone(),
            self.layer.clone(),
            self.total_imports.to_string(),
            self.downward_imports.to_string(),
            self.upward_imports.to_string(),
            self.self_imports.to_string(),
            format!("{:.0}%", self.compliance * 100.0),
        ]
    }
}

/// Per-layer summary.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct LayerSummary {
    /// Layer name
    pub layer: String,
    /// Number of modules in this layer
    pub module_count: usize,
    /// Average depth of modules in this layer
    pub avg_depth: f64,
    /// Average compliance across modules in this layer
    pub avg_compliance: f64,
    /// Total upward imports from this layer
    pub upward_import_count: usize,
}

impl RankEntry for LayerSummary {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Layer"),
            Column::right("Modules"),
            Column::right("Avg Depth"),
            Column::right("Compliance"),
            Column::right("Upward"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            self.layer.clone(),
            self.module_count.to_string(),
            format!("{:.1}", self.avg_depth),
            format!("{:.0}%", self.avg_compliance * 100.0),
            self.upward_import_count.to_string(),
        ]
    }
}

/// Aggregate stats.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LayeringStats {
    pub total_files: usize,
    pub total_layers: usize,
    pub avg_compliance: f64,
    pub worst_layer: String,
    pub worst_compliance: f64,
}

/// Full layering report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LayeringReport {
    pub modules: Vec<LayeringEntry>,
    pub layers: Vec<LayerSummary>,
    pub stats: LayeringStats,
}

/// Analyze import layering compliance.
pub async fn analyze_layering(
    idx: &FileIndex,
    limit: usize,
) -> Result<LayeringReport, libsql::Error> {
    let graph = build_import_graph(idx).await?;

    // Collect all modules
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

    // Assign layers and compute per-layer average depth
    let mut layer_depths: HashMap<String, Vec<f64>> = HashMap::new();
    for module in &all_modules {
        let layer = extract_layer(module);
        let depth = depth_memo.get(module).copied().unwrap_or(0) as f64;
        layer_depths.entry(layer).or_default().push(depth);
    }

    let layer_avg_depth: HashMap<String, f64> = layer_depths
        .iter()
        .map(|(layer, depths)| {
            let avg = depths.iter().sum::<f64>() / depths.len() as f64;
            (layer.clone(), avg)
        })
        .collect();

    // Classify imports for each module using the extracted algorithm
    let raw_entries =
        compute_layering_compliance(&graph.imports_by_file, &all_modules, &layer_avg_depth);

    let mut entries: Vec<LayeringEntry> = raw_entries
        .into_iter()
        .map(|r| LayeringEntry {
            module: r.module,
            layer: r.layer,
            total_imports: r.total_imports,
            downward_imports: r.downward_imports,
            upward_imports: r.upward_imports,
            self_imports: r.self_imports,
            compliance: r.compliance,
        })
        .collect();

    // Build per-layer summaries
    let mut layer_entries: HashMap<String, Vec<&LayeringEntry>> = HashMap::new();
    for entry in &entries {
        layer_entries
            .entry(entry.layer.clone())
            .or_default()
            .push(entry);
    }

    let mut layers: Vec<LayerSummary> = layer_entries
        .iter()
        .map(|(layer, members)| {
            let module_count = members.len();
            let avg_depth = layer_avg_depth.get(layer).copied().unwrap_or(0.0);
            let avg_compliance =
                members.iter().map(|e| e.compliance).sum::<f64>() / module_count as f64;
            let upward_import_count: usize = members.iter().map(|e| e.upward_imports).sum();

            LayerSummary {
                layer: layer.clone(),
                module_count,
                avg_depth,
                avg_compliance,
                upward_import_count,
            }
        })
        .collect();

    layers.sort_by(|a, b| {
        a.avg_compliance
            .partial_cmp(&b.avg_compliance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.upward_import_count.cmp(&a.upward_import_count))
            .then(a.layer.cmp(&b.layer))
    });

    // Stats
    let total_files = entries.len();
    let total_layers = layers.len();
    let avg_compliance = if total_files > 0 {
        entries.iter().map(|e| e.compliance).sum::<f64>() / total_files as f64
    } else {
        1.0
    };
    let (worst_layer, worst_compliance) = layers
        .first()
        .map(|l| (l.layer.clone(), l.avg_compliance))
        .unwrap_or_else(|| ("(none)".to_string(), 1.0));

    let stats = LayeringStats {
        total_files,
        total_layers,
        avg_compliance,
        worst_layer,
        worst_compliance,
    };

    entries.truncate(limit);

    Ok(LayeringReport {
        modules: entries,
        layers,
        stats,
    })
}

impl OutputFormatter for LayeringReport {
    fn format_text(&self) -> String {
        let empty_msg = "No import data found. Run `normalize structure rebuild` first.";
        let mut out = format_ranked_table(
            &format!(
                "# Layering — {} files, {} layers, avg compliance {:.0}%, worst: {} ({:.0}%)",
                self.stats.total_files,
                self.stats.total_layers,
                self.stats.avg_compliance * 100.0,
                self.stats.worst_layer,
                self.stats.worst_compliance * 100.0,
            ),
            &self.modules,
            Some(empty_msg),
        );

        if !self.layers.is_empty() {
            out.push_str("\n\n");
            out.push_str(&format_ranked_table("## Layer Summary", &self.layers, None));
        }

        out
    }

    fn format_pretty(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "\x1b[1;36m# Layering\x1b[0m — \x1b[1m{}\x1b[0m files, \x1b[1m{}\x1b[0m layers, avg compliance \x1b[1m{:.0}%\x1b[0m, worst: \x1b[1;31m{}\x1b[0m (\x1b[31m{:.0}%\x1b[0m)",
            self.stats.total_files,
            self.stats.total_layers,
            self.stats.avg_compliance * 100.0,
            self.stats.worst_layer,
            self.stats.worst_compliance * 100.0,
        ));
        out.push(String::new());

        if self.modules.is_empty() {
            out.push("No import data found. Run `normalize structure rebuild` first.".to_string());
            return out.join("\n");
        }

        let max_mod_len = self
            .modules
            .iter()
            .map(|e| e.module.len())
            .max()
            .unwrap_or(10)
            .min(50);
        let max_layer_len = self
            .modules
            .iter()
            .map(|e| e.layer.len())
            .max()
            .unwrap_or(5)
            .min(20);

        out.push(format!(
            "\x1b[1m{:<mw$}  {:<lw$}  {:>5}  {:>4}  {:>4}  {:>4}  {:>10}\x1b[0m",
            "Module",
            "Layer",
            "Cross",
            "Down",
            "Up",
            "Self",
            "Compliance",
            mw = max_mod_len,
            lw = max_layer_len,
        ));
        out.push(format!(
            "{}--{}-------------------------------",
            "-".repeat(max_mod_len),
            "-".repeat(max_layer_len),
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
            let layer = if entry.layer.len() > max_layer_len {
                format!(
                    "...{}",
                    &entry.layer[entry.layer.len() - (max_layer_len - 3)..]
                )
            } else {
                entry.layer.clone()
            };

            let compliance_color = if entry.compliance >= 0.8 {
                "\x1b[32m"
            } else if entry.compliance >= 0.5 {
                "\x1b[33m"
            } else {
                "\x1b[1;31m"
            };

            let up_color = if entry.upward_imports > 0 {
                "\x1b[31m"
            } else {
                "\x1b[0m"
            };

            out.push(format!(
                "{:<mw$}  {:<lw$}  {:>5}  {:>4}  {}{:>4}\x1b[0m  {:>4}  {}{:>9.0}%\x1b[0m",
                module,
                layer,
                entry.total_imports,
                entry.downward_imports,
                up_color,
                entry.upward_imports,
                entry.self_imports,
                compliance_color,
                entry.compliance * 100.0,
                mw = max_mod_len,
                lw = max_layer_len,
            ));
        }

        // Layer summary
        out.push(String::new());
        out.push("\x1b[1;36m## Layer Summary\x1b[0m".to_string());
        out.push(String::new());

        let max_lname = self
            .layers
            .iter()
            .map(|l| l.layer.len())
            .max()
            .unwrap_or(5)
            .min(20);

        out.push(format!(
            "\x1b[1m{:<lw$}  {:>7}  {:>9}  {:>10}  {:>8}\x1b[0m",
            "Layer",
            "Modules",
            "Avg Depth",
            "Compliance",
            "Upward",
            lw = max_lname,
        ));
        out.push(format!(
            "{}----------------------------------------------",
            "-".repeat(max_lname),
        ));

        for layer in &self.layers {
            let compliance_color = if layer.avg_compliance >= 0.8 {
                "\x1b[32m"
            } else if layer.avg_compliance >= 0.5 {
                "\x1b[33m"
            } else {
                "\x1b[1;31m"
            };

            out.push(format!(
                "{:<lw$}  {:>7}  {:>9.1}  {}{:>9.0}%\x1b[0m  {:>8}",
                layer.layer,
                layer.module_count,
                layer.avg_depth,
                compliance_color,
                layer.avg_compliance * 100.0,
                layer.upward_import_count,
                lw = max_lname,
            ));
        }

        out.join("\n")
    }
}
