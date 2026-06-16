use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::commands::analyze::ceremony::analyze_ceremony;
use crate::commands::analyze::density::analyze_density;
use crate::commands::analyze::test_ratio::{analyze_test_ratio, discover_module_dirs, module_key};
use crate::commands::analyze::uniqueness::analyze_uniqueness;
use crate::output::OutputFormatter;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};

/// Per-module health score and metrics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleHealthEntry {
    pub module: String,
    pub score: f64,
    pub total_lines: usize,
    /// test / (impl + test)
    pub test_ratio: f64,
    /// fraction of functions with no structural near-twin
    pub uniqueness_ratio: f64,
    /// (compression + token uniqueness) / 2
    pub density_score: f64,
    /// interface impl / total callables
    pub ceremony_ratio: f64,
    /// fraction of lines classified as business logic
    pub logic_pct: f64,
}

impl RankEntry for ModuleHealthEntry {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Score"),
            Column::right("Test"),
            Column::right("Uniqueness"),
            Column::right("Density"),
            Column::right("Ceremony"),
            Column::right("Logic"),
            Column::right("Lines"),
            Column::left("Module"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            format!("{:.0}%", self.score * 100.0),
            format!("{:.0}%", self.test_ratio * 100.0),
            format!("{:.0}%", self.uniqueness_ratio * 100.0),
            format!("{:.3}", self.density_score),
            format!("{:.0}%", self.ceremony_ratio * 100.0),
            format!("{:.0}%", self.logic_pct * 100.0),
            self.total_lines.to_string(),
            self.module.clone(),
        ]
    }
}

/// Report returned by `analyze module-health`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleHealthReport {
    pub root: String,
    pub modules_scored: usize,
    /// Sorted by score ascending (worst first).
    pub modules: Vec<ModuleHealthEntry>,
}

impl OutputFormatter for ModuleHealthReport {
    fn format_text(&self) -> String {
        let title = format!(
            "# Module Health — {}, {} modules scored",
            self.root, self.modules_scored
        );
        format_ranked_table(&title, &self.modules, Some("No modules found."))
    }

    fn format_pretty(&self) -> String {
        let title = format!(
            "# Module Health — {}, {} modules scored",
            self.root, self.modules_scored
        );
        crate::output::pretty_ranked_table(&title, &self.modules, Some("No modules found."), |e| {
            use crate::output::tier_color;
            use normalize_analyze::ranked::RiskTier;
            let tier = if e.score >= 0.75 {
                RiskTier::Low
            } else if e.score >= 0.55 {
                RiskTier::Moderate
            } else {
                RiskTier::High
            };
            Some(tier_color(tier))
        })
    }
}

fn score_test(ratio: f64) -> f64 {
    if ratio >= 0.30 {
        1.0
    } else if ratio >= 0.20 {
        0.9
    } else if ratio >= 0.10 {
        0.7
    } else if ratio >= 0.05 {
        0.5
    } else {
        0.3
    }
}

fn score_uniqueness(ratio: f64) -> f64 {
    if ratio >= 0.95 {
        1.0
    } else if ratio >= 0.90 {
        0.9
    } else if ratio >= 0.80 {
        0.7
    } else if ratio >= 0.70 {
        0.5
    } else {
        0.3
    }
}

fn score_density(d: f64) -> f64 {
    if d >= 0.45 {
        1.0
    } else if d >= 0.40 {
        0.9
    } else if d >= 0.35 {
        0.8
    } else if d >= 0.30 {
        0.6
    } else {
        0.4
    }
}

/// Weights: test 35%, uniqueness 35%, density 30%.
/// Ceremony is shown informationally — it's too design-dependent to penalise.
fn module_score(test: f64, uniq: f64, density: f64) -> f64 {
    score_test(test) * 0.35 + score_uniqueness(uniq) * 0.35 + score_density(density) * 0.30
}

/// Analyze per-module health by joining test-ratio, uniqueness, density, and ceremony.
pub fn analyze_module_health(root: &Path, limit: usize, min_lines: usize) -> ModuleHealthReport {
    let module_dirs = discover_module_dirs(root);
    // Run all analyses in parallel.
    let ((test_rep, density_rep), (uniqueness_rep, ceremony_rep)) = rayon::join(
        || {
            rayon::join(
                || analyze_test_ratio(root, 0),
                || analyze_density(root, 0, 0),
            )
        },
        || {
            rayon::join(
                || analyze_uniqueness(root, 0.80, 10, false, false, 0, 0, None),
                || analyze_ceremony(root, 0),
            )
        },
    );

    // Index test ratios by module key (TestRatioEntry.path is already a module key).
    let test_map: HashMap<String, f64> = test_rep
        .entries
        .iter()
        .map(|e| (e.path.clone(), e.ratio))
        .collect();

    // Index density by module.
    let density_map: HashMap<String, f64> = density_rep
        .modules
        .iter()
        .map(|m| (m.module.clone(), m.density_score))
        .collect();

    // Index uniqueness by module.
    let uniqueness_map: HashMap<String, f64> = uniqueness_rep
        .modules
        .iter()
        .map(|m| (m.module.clone(), m.uniqueness_ratio))
        .collect();

    // Index total lines by module from uniqueness (most complete source).
    let lines_map: HashMap<String, usize> = uniqueness_rep
        .modules
        .iter()
        .map(|m| (m.module.clone(), m.total_lines))
        .collect();

    // Aggregate ceremony per module from per-file data.
    // key -> (total_callables, interface_impl)
    let mut ceremony_acc: HashMap<String, (usize, usize)> = HashMap::new();
    for f in &ceremony_rep.top_files {
        let key = module_key(&f.file_path, &module_dirs);
        let entry = ceremony_acc.entry(key).or_default();
        entry.0 += f.total;
        entry.1 += f.interface_impl;
    }
    let ceremony_map: HashMap<String, f64> = ceremony_acc
        .into_iter()
        .map(|(k, (total, impl_))| {
            let ratio = if total > 0 {
                impl_ as f64 / total as f64
            } else {
                0.0
            };
            (k, ratio)
        })
        .collect();

    // Aggregate logic_pct per module from density (total_lines) and test_ratio
    // (impl vs test lines). We can derive logic_pct from test_ratio entries.
    // For simplicity use test_ratio's impl_lines as proxy: logic_pct ≈ impl/(impl+test).
    // (Budget logic_pct would be more accurate but requires another pass.)
    let logic_map: HashMap<String, f64> = test_rep
        .entries
        .iter()
        .map(|e| {
            let total = e.impl_lines + e.test_lines;
            let logic = if total > 0 {
                e.impl_lines as f64 / total as f64
            } else {
                0.0
            };
            (e.path.clone(), logic)
        })
        .collect();

    // Union of all module keys.
    let mut all_modules: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for k in test_map.keys() {
        all_modules.insert(k.clone());
    }
    for k in density_map.keys() {
        all_modules.insert(k.clone());
    }
    for k in uniqueness_map.keys() {
        all_modules.insert(k.clone());
    }

    let mut entries: Vec<ModuleHealthEntry> = all_modules
        .into_iter()
        .filter_map(|module| {
            let total_lines = lines_map.get(&module).copied().unwrap_or(0);
            if total_lines < min_lines {
                return None;
            }
            let test_ratio = test_map.get(&module).copied().unwrap_or(0.0);
            let density = density_map.get(&module).copied().unwrap_or(0.50);
            let uniqueness = uniqueness_map.get(&module).copied().unwrap_or(1.0);
            let ceremony = ceremony_map.get(&module).copied().unwrap_or(0.0);
            let logic_pct = logic_map.get(&module).copied().unwrap_or(0.0);
            let score = module_score(test_ratio, uniqueness, density);
            Some(ModuleHealthEntry {
                module,
                score,
                total_lines,
                test_ratio,
                uniqueness_ratio: uniqueness,
                density_score: density,
                ceremony_ratio: ceremony,
                logic_pct,
            })
        })
        .collect();

    let modules_scored = entries.len();

    normalize_analyze::ranked::rank_and_truncate(
        &mut entries,
        limit,
        |a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.total_lines.cmp(&a.total_lines))
        },
        |e| e.score,
    );

    ModuleHealthReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        modules_scored,
        modules: entries,
    }
}
