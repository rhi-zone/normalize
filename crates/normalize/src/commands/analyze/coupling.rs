//! Temporal coupling analysis — find files that frequently change together

use super::git_utils;
use super::is_source_file;
use crate::output::OutputFormatter;
use glob::Pattern;
use normalize_analyze::ranked::{
    Column, DiffableRankEntry, RankEntry, format_delta, format_ranked_table,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// A pair of files with temporal coupling
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CoupledPair {
    pub file_a: String,
    pub file_b: String,
    /// Number of commits where both files changed
    pub shared_commits: usize,
    /// Total commits touching file_a
    pub commits_a: usize,
    /// Total commits touching file_b
    pub commits_b: usize,
    /// Confidence: shared / max(commits_a, commits_b)
    pub confidence: f64,
    /// Composite key for diff matching (file_a::file_b).
    #[serde(skip)]
    pub pair_key: String,
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl RankEntry for CoupledPair {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("File A"),
            Column::left("File B"),
            Column::right("Shared"),
            Column::right("Conf%"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let shared_str = match self.delta {
            Some(d) => format!("{} ({})", self.shared_commits, format_delta(d, false)),
            None => self.shared_commits.to_string(),
        };
        vec![
            self.file_a.clone(),
            self.file_b.clone(),
            shared_str,
            format!("{:.0}%", self.confidence * 100.0),
        ]
    }
}

impl DiffableRankEntry for CoupledPair {
    fn diff_key(&self) -> &str {
        &self.pair_key
    }
    fn diff_score(&self) -> f64 {
        self.shared_commits as f64
    }
    fn set_delta(&mut self, delta: Option<f64>) {
        self.delta = delta;
    }
    fn delta(&self) -> Option<f64> {
        self.delta
    }
}

/// Per-repo coupling entry for multi-repo runs
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CouplingRepoEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub pairs: Vec<CoupledPair>,
}

/// Temporal coupling report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CouplingReport {
    pub pairs: Vec<CoupledPair>,
    /// Per-repo results when run with --repos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<CouplingRepoEntry>>,
    /// Set when `--diff` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_ref: Option<String>,
}

fn format_coupling_data(pairs: &[CoupledPair]) -> String {
    let mut out = format_ranked_table(
        "# Temporal Coupling (files that change together)",
        pairs,
        Some("No temporal coupling found (no files change together frequently)"),
    );
    out.push_str("\n\nConfidence = shared commits / max(commits_a, commits_b)\nHigh coupling may indicate hidden dependencies or shotgun surgery.");
    out
}

impl OutputFormatter for CouplingReport {
    fn format_text(&self) -> String {
        if let Some(ref repos) = self.repos {
            let mut parts = Vec::new();
            for entry in repos {
                parts.push(format!("=== {} ===", entry.name));
                if let Some(ref err) = entry.error {
                    parts.push(format!("Error: {}", err));
                } else if entry.pairs.is_empty() {
                    parts.push(
                        "No temporal coupling found (no files change together frequently)"
                            .to_string(),
                    );
                } else {
                    parts.push(format_coupling_data(&entry.pairs));
                }
                parts.push(String::new());
            }
            return parts.join("\n");
        }

        if self.pairs.is_empty() {
            return "No temporal coupling found (no files change together frequently)".to_string();
        }
        format_coupling_data(&self.pairs)
    }
}

/// Analyze temporal coupling, returning the report.
pub fn analyze_coupling(
    root: &Path,
    min_commits: usize,
    limit: usize,
    exclude_patterns: &[String],
) -> Result<CouplingReport, String> {
    let excludes: Vec<Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    // Get per-commit file lists via gix — no PATH dependency on git binary.
    let raw_commits = git_utils::git_per_commit_files(root);
    if raw_commits.is_empty() {
        return Err("Not a git repository or no commit history".to_string());
    }

    // Filter each commit's file list to source files that still exist on disk.
    let commits: Vec<Vec<String>> = raw_commits
        .into_iter()
        .map(|files| {
            files
                .into_iter()
                .filter(|f| {
                    let p = Path::new(f.as_str());
                    is_source_file(p)
                        && !excludes.iter().any(|pat| pat.matches(f))
                        && root.join(p).exists()
                })
                .collect::<Vec<_>>()
        })
        .filter(|f| !f.is_empty())
        .collect();

    // Count per-file total commits
    let mut file_commits: HashMap<String, usize> = HashMap::new();
    for commit_files in &commits {
        for f in commit_files {
            *file_commits.entry(f.clone()).or_default() += 1;
        }
    }

    // Count co-changes (pairs that appear in the same commit)
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
    for commit_files in &commits {
        // Skip merge commits or huge commits (likely automated)
        if commit_files.len() > 50 || commit_files.len() < 2 {
            continue;
        }
        let mut sorted = commit_files.clone();
        sorted.sort();
        sorted.dedup();
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                let key = (sorted[i].clone(), sorted[j].clone());
                *pair_counts.entry(key).or_default() += 1;
            }
        }
    }

    // Build pairs above threshold
    let mut pairs: Vec<CoupledPair> = pair_counts
        .into_iter()
        .filter(|(_, count)| *count >= min_commits)
        .map(|((a, b), shared)| {
            let ca = file_commits.get(&a).copied().unwrap_or(0);
            let cb = file_commits.get(&b).copied().unwrap_or(0);
            let confidence = shared as f64 / ca.max(cb) as f64;
            let pair_key = format!("{}::{}", a, b);
            CoupledPair {
                file_a: a,
                file_b: b,
                shared_commits: shared,
                commits_a: ca,
                commits_b: cb,
                confidence,
                pair_key,
                delta: None,
            }
        })
        .collect();

    // Sort by shared commits descending, then confidence descending
    normalize_analyze::ranked::rank_and_truncate(
        &mut pairs,
        limit,
        |a, b| {
            b.shared_commits.cmp(&a.shared_commits).then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        },
        |p| p.shared_commits as f64,
    );

    Ok(CouplingReport {
        pairs,
        repos: None,
        diff_ref: None,
    })
}
