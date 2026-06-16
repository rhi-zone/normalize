//! Dependency depth map: per-module depth in the import stack + ripple risk.
//!
//! Depth 0 = entry point (nothing imports it). High depth = deeply embedded.
//! Ripple score = fan_out × depth × downstream — composite blast radius metric.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use normalize_analyze::ranked::{
    Column, DiffableRankEntry, RankEntry, format_delta, format_ranked_table,
};
use normalize_architecture::{build_import_graph, compute_depth, compute_downstream};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// One module's depth and risk metrics.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
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
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl RankEntry for DepthEntry {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Module"),
            Column::right("Depth"),
            Column::right("Fan-in"),
            Column::right("Fan-out"),
            Column::right("Downstream"),
            Column::right("Ripple Score"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            self.module.clone(),
            self.depth.to_string(),
            self.fan_in.to_string(),
            self.fan_out.to_string(),
            self.downstream.to_string(),
            match self.delta {
                Some(d) => format!("{} ({})", self.ripple_score, format_delta(d, false)),
                None => self.ripple_score.to_string(),
            },
        ]
    }
}

impl DiffableRankEntry for DepthEntry {
    fn diff_key(&self) -> &str {
        &self.module
    }
    fn diff_score(&self) -> f64 {
        self.ripple_score as f64
    }
    fn set_delta(&mut self, delta: Option<f64>) {
        self.delta = delta;
    }
    fn delta(&self) -> Option<f64> {
        self.delta
    }
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
    /// Set when `--diff` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_ref: Option<String>,
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
                delta: None,
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
        diff_ref: None,
    })
}

impl OutputFormatter for DepthMapReport {
    fn format_text(&self) -> String {
        format_ranked_table(
            &format!(
                "# Depth Map — {} modules, max depth {}, avg {:.1}, {} entry points",
                self.stats.total_modules,
                self.stats.max_depth,
                self.stats.avg_depth,
                self.stats.modules_at_depth_0,
            ),
            &self.modules,
            Some("No import data found. Run `normalize structure rebuild` first."),
        )
    }

    fn format_pretty(&self) -> String {
        crate::output::pretty_ranked_table(
            &format!(
                "# Depth Map — {} modules, max depth {}, avg {:.1}, {} entry points",
                self.stats.total_modules,
                self.stats.max_depth,
                self.stats.avg_depth,
                self.stats.modules_at_depth_0,
            ),
            &self.modules,
            Some("No import data found. Run `normalize structure rebuild` first."),
            |_| None,
        )
    }
}
