//! Git history hotspot analysis

use super::git_utils;
use super::is_source_file;
use crate::analyze::complexity::ComplexityAnalyzer;
use crate::output::OutputFormatter;
use glob::Pattern;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Hotspot data for a file
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileHotspot {
    pub path: String,
    pub commits: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub max_complexity: Option<usize>,
    #[serde(skip)]
    pub score: f64,
}

impl RankEntry for FileHotspot {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Score"),
            Column::right("Commits"),
            Column::right("Churn"),
            Column::right("Complexity"),
            Column::left("File"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let churn = self.lines_added + self.lines_deleted;
        let cplx = match self.max_complexity {
            Some(c) => c.to_string(),
            None => "-".to_string(),
        };
        vec![
            format!("{:.0}", self.score),
            self.commits.to_string(),
            churn.to_string(),
            cplx,
            self.path.clone(),
        ]
    }
}

/// Hotspot data for a file (without complexity column).
struct FileHotspotNoComplexity<'a>(&'a FileHotspot);

impl RankEntry for FileHotspotNoComplexity<'_> {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Score"),
            Column::right("Commits"),
            Column::right("Churn"),
            Column::left("File"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let h = self.0;
        let churn = h.lines_added + h.lines_deleted;
        vec![
            format!("{:.0}", h.score),
            h.commits.to_string(),
            churn.to_string(),
            h.path.clone(),
        ]
    }
}

/// Per-repo hotspots entry for multi-repo runs
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HotspotsRepoEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub hotspots: Vec<FileHotspot>,
    pub has_complexity: bool,
    pub recency_weighted: bool,
}

/// Hotspots analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HotspotsReport {
    pub hotspots: Vec<FileHotspot>,
    pub has_complexity: bool,
    pub recency_weighted: bool,
    /// Per-repo results when run with --repos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<HotspotsRepoEntry>>,
}

fn hotspots_title(recency_weighted: bool) -> &'static str {
    if recency_weighted {
        "# Git Hotspots (recency-weighted churn)"
    } else {
        "# Git Hotspots (high churn)"
    }
}

fn format_hotspots_data(
    hotspots: &[FileHotspot],
    has_complexity: bool,
    recency_weighted: bool,
) -> String {
    let title = hotspots_title(recency_weighted);
    if has_complexity {
        format_ranked_table(title, hotspots, None)
    } else {
        let entries: Vec<FileHotspotNoComplexity<'_>> =
            hotspots.iter().map(FileHotspotNoComplexity).collect();
        format_ranked_table(title, &entries, None)
    }
}

fn pretty_hotspots_data(
    hotspots: &[FileHotspot],
    has_complexity: bool,
    recency_weighted: bool,
) -> String {
    let title = hotspots_title(recency_weighted);
    if has_complexity {
        crate::output::pretty_ranked_table(title, hotspots, None, |_| None)
    } else {
        let entries: Vec<FileHotspotNoComplexity<'_>> =
            hotspots.iter().map(FileHotspotNoComplexity).collect();
        crate::output::pretty_ranked_table(title, &entries, None, |_| None)
    }
}

impl OutputFormatter for HotspotsReport {
    fn format_text(&self) -> String {
        if let Some(ref repos) = self.repos {
            let mut parts = Vec::new();
            for entry in repos {
                parts.push(format!("=== {} ===", entry.name));
                if let Some(ref err) = entry.error {
                    parts.push(format!("Error: {}", err));
                } else if entry.hotspots.is_empty() {
                    parts.push("No hotspots found (no git history or source files)".to_string());
                } else {
                    parts.push(format_hotspots_data(
                        &entry.hotspots,
                        entry.has_complexity,
                        entry.recency_weighted,
                    ));
                }
                parts.push(String::new());
            }
            return parts.join("\n");
        }

        if self.hotspots.is_empty() {
            return "No hotspots found (no git history or source files)".to_string();
        }
        format_hotspots_data(&self.hotspots, self.has_complexity, self.recency_weighted)
    }

    fn format_pretty(&self) -> String {
        if let Some(ref repos) = self.repos {
            let mut parts = Vec::new();
            for entry in repos {
                parts.push(format!("=== {} ===", entry.name));
                if let Some(ref err) = entry.error {
                    parts.push(format!("Error: {}", err));
                } else if entry.hotspots.is_empty() {
                    parts.push("No hotspots found (no git history or source files)".to_string());
                } else {
                    parts.push(pretty_hotspots_data(
                        &entry.hotspots,
                        entry.has_complexity,
                        entry.recency_weighted,
                    ));
                }
                parts.push(String::new());
            }
            return parts.join("\n");
        }

        if self.hotspots.is_empty() {
            return "No hotspots found (no git history or source files)".to_string();
        }
        pretty_hotspots_data(&self.hotspots, self.has_complexity, self.recency_weighted)
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

/// Build per-file churn stats via gix (no PATH dependency).
fn parse_git_churn(root: &Path) -> Result<HashMap<String, ChurnStats>, String> {
    let raw = git_utils::git_file_churn_stats(root);
    if raw.is_empty() {
        // Check whether this is a genuine "no history" case or "not a repo" case.
        if git_utils::open_repo(root).is_none() {
            return Err("Not a git repository".to_string());
        }
        return Err("git log failed or no history found".to_string());
    }
    let stats = raw
        .into_iter()
        .map(|(path, entries)| {
            let commits = entries
                .into_iter()
                .map(|e| CommitChurn {
                    added: e.added,
                    deleted: e.deleted,
                    timestamp: e.timestamp,
                })
                .collect();
            (path, ChurnStats { commits })
        })
        .collect();
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

    // parse_git_churn (via gix) returns an error if not a git repository.
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

    normalize_analyze::ranked::rank_and_truncate(
        &mut hotspots,
        20,
        |a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        },
        |h| h.score,
    );

    Ok(HotspotsReport {
        hotspots,
        has_complexity,
        recency_weighted: recency,
        repos: None,
    })
}
