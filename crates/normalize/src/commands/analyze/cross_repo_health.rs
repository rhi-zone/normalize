//! Cross-repo tech debt health: unified ranking combining churn, complexity, and coupling.

use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::Path;

/// Health metrics for a single repo
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoHealthEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    // Raw churn data
    pub churn_score: f64,
    pub hotspot_files: usize,

    // Raw complexity data
    pub avg_complexity: f64,
    pub max_complexity: usize,
    pub critical_functions: usize,
    pub high_functions: usize,

    // Raw coupling data
    pub coupling_pairs: usize,
    pub avg_coupling_confidence: f64,

    /// Normalized score 0–100 (higher = more tech debt)
    pub tech_debt_score: f64,
}

/// Cross-repo tech debt leaderboard
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CrossRepoHealthReport {
    /// Repos sorted by tech_debt_score descending
    pub repos: Vec<RepoHealthEntry>,
}

impl OutputFormatter for CrossRepoHealthReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Cross-Repo Tech Debt Health".to_string());
        lines.push(String::new());

        if self.repos.is_empty() {
            lines.push("No repos analyzed.".to_string());
            return lines.join("\n");
        }

        lines.push(format!(
            "{:<30} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7}",
            "Repo", "Score", "Churn", "AvgCx", "Critical", "High", "Pairs"
        ));
        lines.push("-".repeat(82));

        for entry in &self.repos {
            if let Some(err) = &entry.error {
                lines.push(format!("{:<30} ERROR: {}", entry.name, err));
                continue;
            }
            lines.push(format!(
                "{:<30} {:>6.1} {:>8.1} {:>8.2} {:>8} {:>7} {:>7}",
                entry.name,
                entry.tech_debt_score,
                entry.churn_score,
                entry.avg_complexity,
                entry.critical_functions,
                entry.high_functions,
                entry.coupling_pairs,
            ));
        }

        lines.push(String::new());
        lines.push("Columns: Score=composite 0-100 | Churn=hotspot sum | AvgCx=avg cyclomatic | Critical/High=functions | Pairs=coupled pairs".to_string());

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push("\x1b[1mCross-Repo Tech Debt Health\x1b[0m".to_string());
        lines.push(String::new());

        if self.repos.is_empty() {
            lines.push("No repos analyzed.".to_string());
            return lines.join("\n");
        }

        lines.push(format!(
            "{:<30} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7}",
            "\x1b[2mRepo\x1b[0m",
            "\x1b[2mScore\x1b[0m",
            "\x1b[2mChurn\x1b[0m",
            "\x1b[2mAvgCx\x1b[0m",
            "\x1b[2mCritical\x1b[0m",
            "\x1b[2mHigh\x1b[0m",
            "\x1b[2mPairs\x1b[0m",
        ));
        lines.push("\x1b[2m".to_string() + &"-".repeat(82) + "\x1b[0m");

        for (i, entry) in self.repos.iter().enumerate() {
            if let Some(err) = &entry.error {
                lines.push(format!("{:<30} \x1b[31mERROR: {}\x1b[0m", entry.name, err));
                continue;
            }

            let score_color = if entry.tech_debt_score >= 70.0 {
                "\x1b[31m" // red
            } else if entry.tech_debt_score >= 40.0 {
                "\x1b[33m" // yellow
            } else {
                "\x1b[32m" // green
            };

            let rank = i + 1;
            lines.push(format!(
                "{:<30} {}{:>6.1}\x1b[0m {:>8.1} {:>8.2} {:>8} {:>7} {:>7}",
                format!("{}. {}", rank, entry.name),
                score_color,
                entry.tech_debt_score,
                entry.churn_score,
                entry.avg_complexity,
                entry.critical_functions,
                entry.high_functions,
                entry.coupling_pairs,
            ));
        }

        lines.push(String::new());
        lines.push(
            "\x1b[2mScore = 40% churn + 40% complexity + 20% coupling (normalized 0–100)\x1b[0m"
                .to_string(),
        );

        lines.join("\n")
    }
}

/// Per-repo raw metrics before normalization
struct RawMetrics {
    name: String,
    error: Option<String>,
    churn_score: f64,
    hotspot_files: usize,
    avg_complexity: f64,
    max_complexity: usize,
    critical_functions: usize,
    high_functions: usize,
    coupling_pairs: usize,
    avg_coupling_confidence: f64,
}

fn gather_metrics(repo_path: &Path) -> RawMetrics {
    let name = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // --- Churn (hotspots) ---
    let config = crate::config::NormalizeConfig::load(repo_path);
    let mut hotspot_excludes = config.analyze.hotspots_exclude.clone();
    hotspot_excludes.extend(crate::commands::analyze::load_allow_file(
        repo_path,
        "hotspots-allow",
    ));

    let (churn_score, hotspot_files, churn_error) =
        match crate::commands::analyze::hotspots::analyze_hotspots(
            repo_path,
            &hotspot_excludes,
            false,
        ) {
            Ok(r) => {
                let total: f64 = r.hotspots.iter().map(|h| h.score).sum();
                let count = r.hotspots.len();
                (total, count, None)
            }
            Err(e) => (0.0, 0, Some(e)),
        };

    if let Some(err) = churn_error {
        return RawMetrics {
            name,
            error: Some(err),
            churn_score: 0.0,
            hotspot_files: 0,
            avg_complexity: 0.0,
            max_complexity: 0,
            critical_functions: 0,
            high_functions: 0,
            coupling_pairs: 0,
            avg_coupling_confidence: 0.0,
        };
    }

    // --- Complexity ---
    let complexity_allowlist =
        crate::commands::analyze::load_allow_file(repo_path, "complexity-allow");
    let cx_report = crate::commands::analyze::complexity::analyze_codebase_complexity(
        repo_path,
        usize::MAX,
        None,
        None,
        &complexity_allowlist,
    );
    let (avg_complexity, max_complexity, critical_functions, high_functions) =
        if let Some(stats) = &cx_report.full_stats {
            (
                stats.total_avg,
                stats.total_max,
                stats.critical_count,
                stats.high_count,
            )
        } else {
            (0.0, 0, 0, 0)
        };

    // --- Coupling ---
    let (coupling_pairs, avg_coupling_confidence) =
        match crate::commands::analyze::coupling::analyze_coupling(repo_path, 2, usize::MAX, &[]) {
            Ok(r) => {
                let count = r.pairs.len();
                let avg_conf = if count == 0 {
                    0.0
                } else {
                    r.pairs.iter().map(|p| p.confidence).sum::<f64>() / count as f64
                };
                (count, avg_conf)
            }
            Err(_) => (0, 0.0),
        };

    RawMetrics {
        name,
        error: None,
        churn_score,
        hotspot_files,
        avg_complexity,
        max_complexity,
        critical_functions,
        high_functions,
        coupling_pairs,
        avg_coupling_confidence,
    }
}

fn normalize_vec(values: &[f64]) -> Vec<f64> {
    let max = values.iter().cloned().fold(0.0_f64, f64::max);
    if max == 0.0 {
        vec![0.0; values.len()]
    } else {
        values.iter().map(|v| v / max * 100.0).collect()
    }
}

/// Analyze tech debt health across all given repos.
pub fn analyze_cross_repo_health(repos: &[std::path::PathBuf]) -> CrossRepoHealthReport {
    let raw: Vec<RawMetrics> = repos.iter().map(|p| gather_metrics(p)).collect();

    // Build component vectors (0 for errored repos)
    let churn_vals: Vec<f64> = raw.iter().map(|r| r.churn_score).collect();
    // Complexity component: avg * sqrt(1 + critical) to weight critical functions
    let cx_vals: Vec<f64> = raw
        .iter()
        .map(|r| r.avg_complexity * (1.0 + r.critical_functions as f64).sqrt())
        .collect();
    // Coupling component: pairs * avg_confidence
    let coup_vals: Vec<f64> = raw
        .iter()
        .map(|r| r.coupling_pairs as f64 * r.avg_coupling_confidence.max(0.01))
        .collect();

    let churn_norm = normalize_vec(&churn_vals);
    let cx_norm = normalize_vec(&cx_vals);
    let coup_norm = normalize_vec(&coup_vals);

    let mut repos: Vec<RepoHealthEntry> = raw
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let tech_debt_score = if r.error.is_some() {
                0.0
            } else {
                0.4 * churn_norm[i] + 0.4 * cx_norm[i] + 0.2 * coup_norm[i]
            };

            RepoHealthEntry {
                name: r.name,
                error: r.error,
                churn_score: (r.churn_score * 10.0).round() / 10.0,
                hotspot_files: r.hotspot_files,
                avg_complexity: r.avg_complexity,
                max_complexity: r.max_complexity,
                critical_functions: r.critical_functions,
                high_functions: r.high_functions,
                coupling_pairs: r.coupling_pairs,
                avg_coupling_confidence: r.avg_coupling_confidence,
                tech_debt_score: (tech_debt_score * 10.0).round() / 10.0,
            }
        })
        .collect();

    repos.sort_by(|a, b| b.tech_debt_score.partial_cmp(&a.tech_debt_score).unwrap());

    CrossRepoHealthReport { repos }
}
