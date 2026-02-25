//! Git history hotspot analysis

use super::is_source_file;
use crate::analyze::complexity::ComplexityAnalyzer;
use crate::output::OutputFormatter;
use glob::Pattern;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Hotspot data for a file
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct FileHotspot {
    path: String,
    commits: usize,
    lines_added: usize,
    lines_deleted: usize,
    max_complexity: Option<usize>,
    #[serde(skip)]
    score: f64,
}

/// Hotspots analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HotspotsReport {
    hotspots: Vec<FileHotspot>,
    has_complexity: bool,
    recency_weighted: bool,
}

impl OutputFormatter for HotspotsReport {
    fn format_text(&self) -> String {
        if self.hotspots.is_empty() {
            return "No hotspots found (no git history or source files)".to_string();
        }

        let mut lines = Vec::new();
        let title = if self.recency_weighted {
            "Git Hotspots (recency-weighted churn)"
        } else {
            "Git Hotspots (high churn)"
        };
        lines.push(title.to_string());
        lines.push(String::new());

        if self.has_complexity {
            lines.push(format!(
                "{:<50} {:>8} {:>8} {:>6} {:>8}",
                "File", "Commits", "Churn", "Cplx", "Score"
            ));
            lines.push("-".repeat(86));
        } else {
            lines.push(format!(
                "{:<50} {:>8} {:>8} {:>8}",
                "File", "Commits", "Churn", "Score"
            ));
            lines.push("-".repeat(80));
        }

        for h in &self.hotspots {
            let churn = h.lines_added + h.lines_deleted;
            let display_path = if h.path.len() > 48 {
                format!("...{}", &h.path[h.path.len() - 45..])
            } else {
                h.path.clone()
            };
            if self.has_complexity {
                let cplx_str = match h.max_complexity {
                    Some(c) => format!("{}", c),
                    None => "-".to_string(),
                };
                lines.push(format!(
                    "{:<50} {:>8} {:>8} {:>6} {:>8.0}",
                    display_path, h.commits, churn, cplx_str, h.score
                ));
            } else {
                lines.push(format!(
                    "{:<50} {:>8} {:>8} {:>8.0}",
                    display_path, h.commits, churn, h.score
                ));
            }
        }

        lines.push(String::new());

        let base_formula = if self.recency_weighted {
            "\u{2211}(e^(-\u{03bb}\u{00b7}age) \u{00d7} \u{221a}churn_i)"
        } else {
            "commits \u{00d7} \u{221a}churn"
        };

        if self.has_complexity {
            lines.push(format!(
                "Score = {} \u{00d7} log\u{2082}(1 + max_complexity)",
                base_formula
            ));
            lines.push(
                "High scores indicate complex, bug-prone files that change often.".to_string(),
            );
        } else {
            lines.push(format!("Score = {}", base_formula));
            lines.push("High scores indicate bug-prone files that change often.".to_string());
            lines.push("Run with complexity data for risk-weighted scores.".to_string());
        }

        if self.recency_weighted {
            lines.push("Recency half-life: 180 days (recent changes weighted higher).".to_string());
        }

        lines.join("\n")
    }
}

/// Per-commit churn entry for a file
struct CommitChurn {
    added: usize,
    deleted: usize,
    /// Unix timestamp of the commit
    timestamp: u64,
}

/// Raw churn stats from git log
struct ChurnStats {
    commits: Vec<CommitChurn>,
}

impl ChurnStats {
    fn total_commits(&self) -> usize {
        self.commits.len()
    }

    fn total_added(&self) -> usize {
        self.commits.iter().map(|c| c.added).sum()
    }

    fn total_deleted(&self) -> usize {
        self.commits.iter().map(|c| c.deleted).sum()
    }
}

/// Parse `git log --pretty=format:%at --numstat` output into per-file churn stats with timestamps
fn parse_git_churn(root: &Path) -> Result<HashMap<String, ChurnStats>, String> {
    let output = std::process::Command::new("git")
        .args(["log", "--pretty=format:%at", "--numstat"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git log: {}", e))?;

    if !output.status.success() {
        return Err("git log failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut stats: HashMap<String, ChurnStats> = HashMap::new();
    let mut current_timestamp: u64 = 0;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try to parse as timestamp (just a number on its own line)
        if let Ok(ts) = trimmed.parse::<u64>() {
            // Timestamps are > 1_000_000_000 (year 2001+), numstat added/deleted are small
            if ts > 1_000_000_000 {
                current_timestamp = ts;
                continue;
            }
        }

        // Try to parse as numstat: added<TAB>deleted<TAB>path
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 3 {
            continue;
        }
        if parts[0] == "-" || parts[1] == "-" {
            continue;
        }
        let added = parts[0].parse::<usize>().unwrap_or(0);
        let deleted = parts[1].parse::<usize>().unwrap_or(0);
        let path = parts[2].to_string();

        let entry = stats.entry(path).or_insert(ChurnStats {
            commits: Vec::new(),
        });
        entry.commits.push(CommitChurn {
            added,
            deleted,
            timestamp: current_timestamp,
        });
    }

    Ok(stats)
}

/// Compute max cyclomatic complexity per file using ComplexityAnalyzer
fn compute_file_complexities(root: &Path, paths: &[String]) -> HashMap<String, usize> {
    let analyzer = ComplexityAnalyzer::new();

    paths
        .par_iter()
        .filter_map(|path| {
            let full_path = root.join(path);
            let content = std::fs::read_to_string(&full_path).ok()?;
            let report = analyzer.analyze(&full_path, &content);
            let max_complexity = report.functions.iter().map(|f| f.complexity).max()?;
            Some((path.clone(), max_complexity))
        })
        .collect()
}

/// Calculate hotspot score without recency weighting
fn hotspot_score(commits: usize, churn: usize, max_complexity: Option<usize>) -> f64 {
    let base = (commits as f64) * (churn as f64).sqrt();
    apply_complexity(base, max_complexity)
}

/// Calculate hotspot score with recency weighting (exponential decay)
///
/// Each commit contributes `e^(-λ × age_days) × √churn_i` instead of flat `commits × √total_churn`.
/// Half-life = 180 days → λ = ln(2)/180
fn hotspot_score_recency(
    commit_churns: &[CommitChurn],
    now: u64,
    max_complexity: Option<usize>,
) -> f64 {
    const HALF_LIFE_DAYS: f64 = 180.0;
    let lambda = std::f64::consts::LN_2 / HALF_LIFE_DAYS;

    let base: f64 = commit_churns
        .iter()
        .map(|c| {
            let age_secs = now.saturating_sub(c.timestamp) as f64;
            let age_days = age_secs / 86400.0;
            let weight = (-lambda * age_days).exp();
            let churn = (c.added + c.deleted) as f64;
            weight * churn.sqrt()
        })
        .sum();

    apply_complexity(base, max_complexity)
}

fn apply_complexity(base: f64, max_complexity: Option<usize>) -> f64 {
    match max_complexity {
        Some(c) if c > 0 => base * (1.0 + c as f64).log2(),
        _ => base,
    }
}

/// Analyze git history hotspots, returning the report.
pub fn analyze_hotspots(
    root: &Path,
    exclude_patterns: &[String],
    recency: bool,
) -> Result<HotspotsReport, String> {
    let excludes: Vec<Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let git_dir = root.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    let file_stats = parse_git_churn(root)?;

    // Filter to existing source files, excluding patterns
    let candidate_paths: Vec<String> = file_stats
        .keys()
        .filter(|path| {
            let p = Path::new(path.as_str());
            root.join(p).exists()
                && is_source_file(p)
                && !excludes.iter().any(|pat| pat.matches(path))
        })
        .cloned()
        .collect();

    // Compute per-file max complexity
    let complexities = compute_file_complexities(root, &candidate_paths);
    let has_complexity = !complexities.is_empty();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut hotspots: Vec<FileHotspot> = candidate_paths
        .into_iter()
        .map(|path| {
            let stats = &file_stats[&path];
            let max_complexity = complexities.get(&path).copied();
            let score = if recency {
                hotspot_score_recency(&stats.commits, now, max_complexity)
            } else {
                let churn = stats.total_added() + stats.total_deleted();
                hotspot_score(stats.total_commits(), churn, max_complexity)
            };
            FileHotspot {
                path,
                commits: stats.total_commits(),
                lines_added: stats.total_added(),
                lines_deleted: stats.total_deleted(),
                max_complexity,
                score,
            }
        })
        .collect();

    hotspots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    hotspots.truncate(20);

    Ok(HotspotsReport {
        hotspots,
        has_complexity,
        recency_weighted: recency,
    })
}

/// Analyze git history hotspots (CLI entry point)
pub fn cmd_hotspots(
    root: &Path,
    exclude_patterns: &[String],
    recency: bool,
    format: &crate::output::OutputFormat,
) -> i32 {
    match analyze_hotspots(root, exclude_patterns, recency) {
        Ok(report) => {
            report.print(format);
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}
