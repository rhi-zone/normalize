//! Temporal coupling analysis — find files that frequently change together

use super::is_source_file;
use crate::output::OutputFormatter;
use glob::Pattern;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};
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
        vec![
            self.file_a.clone(),
            self.file_b.clone(),
            self.shared_commits.to_string(),
            format!("{:.0}%", self.confidence * 100.0),
        ]
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

    let git_dir = root.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    // Get per-commit file lists using --name-only with commit delimiters
    // Use %x00 (null byte) as delimiter since it can't appear in filenames
    let output = std::process::Command::new("git")
        .args(["log", "--pretty=format:%x00", "--name-only"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git log: {}", e))?;

    if !output.status.success() {
        return Err("git log failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse into per-commit file sets
    let mut commits: Vec<Vec<String>> = Vec::new();
    let mut current_files: Vec<String> = Vec::new();

    for line in stdout.lines() {
        if line.contains('\0') {
            if !current_files.is_empty() {
                commits.push(std::mem::take(&mut current_files));
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let p = Path::new(trimmed);
        if !is_source_file(p) {
            continue;
        }
        if excludes.iter().any(|pat| pat.matches(trimmed)) {
            continue;
        }
        // Only include files that still exist
        if root.join(p).exists() {
            current_files.push(trimmed.to_string());
        }
    }
    if !current_files.is_empty() {
        commits.push(current_files);
    }

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
            CoupledPair {
                file_a: a,
                file_b: b,
                shared_commits: shared,
                commits_a: ca,
                commits_b: cb,
                confidence,
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

    Ok(CouplingReport { pairs, repos: None })
}
