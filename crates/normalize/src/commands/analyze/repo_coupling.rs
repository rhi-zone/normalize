//! Cross-repo coupling analysis — dependency graph + temporal coupling signals

use crate::output::OutputFormatter;
use normalize_ecosystems::{DepSource, detect_all_ecosystems};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A directed dependency edge: source depends on target via package_name.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DepEdge {
    pub source_repo: String,
    pub target_repo: String,
    pub package_name: String,
    pub ecosystem: String,
}

/// Temporal coupling between two repos.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TemporalCouplingPair {
    pub repo_a: String,
    pub repo_b: String,
    pub shared_windows: usize,
    pub windows_a: usize,
    pub windows_b: usize,
    pub coupling_ratio: f64,
}

/// Per-repo activity context.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoActivity {
    pub name: String,
    pub ecosystems: Vec<String>,
    pub published_names: Vec<String>,
    pub total_commits: usize,
    pub active_windows: usize,
}

/// Cross-repo coupling report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoCouplingReport {
    pub dep_edges: Vec<DepEdge>,
    pub temporal_pairs: Vec<TemporalCouplingPair>,
    pub undeclared_pairs: Vec<TemporalCouplingPair>,
    pub repos: Vec<RepoActivity>,
    pub window_hours: usize,
}

impl OutputFormatter for RepoCouplingReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        // Section 1: Dependency Graph
        lines.push("Dependency Graph".to_string());
        if self.dep_edges.is_empty() {
            lines.push("  (no cross-repo dependencies found)".to_string());
        } else {
            for edge in &self.dep_edges {
                lines.push(format!(
                    "  {} -> {}  (via \"{}\", {})",
                    edge.source_repo, edge.target_repo, edge.package_name, edge.ecosystem,
                ));
            }
        }

        // Section 2: Temporal Coupling
        lines.push(String::new());
        lines.push(format!(
            "Temporal Coupling ({}h windows)",
            self.window_hours,
        ));
        if self.temporal_pairs.is_empty() {
            lines.push("  (no temporal coupling found)".to_string());
        } else {
            lines.push(format!(
                "  {:<22} {:<22} {:>6} {:>6}",
                "Repo A", "Repo B", "Shared", "Ratio",
            ));
            lines.push(format!("  {}", "-".repeat(60)));
            for pair in &self.temporal_pairs {
                lines.push(format!(
                    "  {:<22} {:<22} {:>6} {:>5.0}%",
                    truncate(&pair.repo_a, 20),
                    truncate(&pair.repo_b, 20),
                    pair.shared_windows,
                    pair.coupling_ratio * 100.0,
                ));
            }
        }

        // Section 3: Undeclared Coupling
        if !self.undeclared_pairs.is_empty() {
            lines.push(String::new());
            lines.push("Undeclared Coupling (temporal without dependency edge)".to_string());
            lines.push(format!(
                "  {:<22} {:<22} {:>6} {:>6}",
                "Repo A", "Repo B", "Shared", "Ratio",
            ));
            lines.push(format!("  {}", "-".repeat(60)));
            for pair in &self.undeclared_pairs {
                lines.push(format!(
                    "  {:<22} {:<22} {:>6} {:>5.0}%",
                    truncate(&pair.repo_a, 20),
                    truncate(&pair.repo_b, 20),
                    pair.shared_windows,
                    pair.coupling_ratio * 100.0,
                ));
            }
        }

        // Section 4: Repo Summary
        lines.push(String::new());
        lines.push("Repo Summary".to_string());
        lines.push(format!(
            "  {:<20} {:<14} {:<20} {:>7} {:>7}",
            "Repo", "Ecosystems", "Packages", "Commits", "Windows",
        ));
        lines.push(format!("  {}", "-".repeat(72)));
        for repo in &self.repos {
            lines.push(format!(
                "  {:<20} {:<14} {:<20} {:>7} {:>7}",
                truncate(&repo.name, 18),
                truncate(&repo.ecosystems.join(","), 12),
                truncate(&repo.published_names.join(","), 18),
                repo.total_commits,
                repo.active_windows,
            ));
        }

        lines.join("\n")
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Analyze cross-repo coupling: dependency graph + temporal signals.
pub fn analyze_repo_coupling(
    repos: &[PathBuf],
    window_hours: usize,
    min_windows: usize,
) -> Result<RepoCouplingReport, String> {
    use rayon::prelude::*;

    if repos.len() < 2 {
        return Err("Need at least 2 repos for coupling analysis".to_string());
    }

    // Phase 1: Gather per-repo data in parallel
    let repo_data: Vec<RepoData> = repos
        .par_iter()
        .filter_map(|r| gather_repo_data(r))
        .collect();

    if repo_data.is_empty() {
        return Err("No analyzable repositories found".to_string());
    }

    // Phase 2: Build dependency graph
    let dep_edges = build_dep_graph(&repo_data);

    // Phase 3: Compute temporal coupling
    let temporal_pairs = compute_temporal_coupling(&repo_data, window_hours, min_windows);

    // Phase 4: Find undeclared coupling
    let declared_pairs: HashSet<(String, String)> = dep_edges
        .iter()
        .flat_map(|e| {
            // Both directions count as declared
            vec![
                (e.source_repo.clone(), e.target_repo.clone()),
                (e.target_repo.clone(), e.source_repo.clone()),
            ]
        })
        .collect();

    let undeclared_pairs: Vec<TemporalCouplingPair> = temporal_pairs
        .iter()
        .filter(|p| {
            !declared_pairs.contains(&(p.repo_a.clone(), p.repo_b.clone()))
                && !declared_pairs.contains(&(p.repo_b.clone(), p.repo_a.clone()))
        })
        .cloned()
        .collect();

    // Build repo summaries
    let repos_summary: Vec<RepoActivity> = repo_data
        .iter()
        .map(|rd| {
            let window_secs = (window_hours as u64) * 3600;
            let windows = count_active_windows(&rd.commit_timestamps, window_secs);
            RepoActivity {
                name: rd.name.clone(),
                ecosystems: rd.ecosystem_names.clone(),
                published_names: rd.published_names.clone(),
                total_commits: rd.commit_timestamps.len(),
                active_windows: windows,
            }
        })
        .collect();

    Ok(RepoCouplingReport {
        dep_edges,
        temporal_pairs,
        undeclared_pairs,
        repos: repos_summary,
        window_hours,
    })
}

// ============================================================================
// Internal types and helpers
// ============================================================================

struct RepoData {
    name: String,
    path: PathBuf,
    ecosystem_names: Vec<String>,
    published_names: Vec<String>,
    /// (dep_name, ecosystem_name, source)
    dependencies: Vec<(String, String, DepSource)>,
    commit_timestamps: Vec<u64>,
    remote_url: Option<String>,
}

fn gather_repo_data(repo: &Path) -> Option<RepoData> {
    let name = repo.file_name()?.to_str()?.to_string();

    // Detect ecosystems and gather deps/published names
    let ecosystems = detect_all_ecosystems(repo);
    let mut ecosystem_names = Vec::new();
    let mut published_names = Vec::new();
    let mut dependencies = Vec::new();

    for eco in &ecosystems {
        ecosystem_names.push(eco.name().to_string());
        published_names.extend(eco.published_names(repo));

        if let Ok(deps) = eco.list_dependencies(repo) {
            for dep in deps {
                dependencies.push((
                    dep.effective_name().to_string(),
                    eco.name().to_string(),
                    dep.source.clone(),
                ));
            }
        }
    }

    // Get commit timestamps
    let commit_timestamps = get_commit_timestamps(repo);

    // Get remote URL
    let remote_url = get_remote_url(repo);

    Some(RepoData {
        name,
        path: repo.to_path_buf(),
        ecosystem_names,
        published_names,
        dependencies,
        commit_timestamps,
        remote_url,
    })
}

fn get_commit_timestamps(repo: &Path) -> Vec<u64> {
    let output = std::process::Command::new("git")
        .args(["log", "--pretty=format:%at"])
        .current_dir(repo)
        .output()
        .ok();

    match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u64>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn get_remote_url(repo: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo)
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() {
            return Some(url);
        }
    }
    None
}

fn build_dep_graph(repos: &[RepoData]) -> Vec<DepEdge> {
    // Build lookup indexes
    // 1. package_name -> repo_name
    let mut name_to_repo: HashMap<String, String> = HashMap::new();
    for rd in repos {
        for pname in &rd.published_names {
            name_to_repo.insert(pname.clone(), rd.name.clone());
        }
    }

    // 2. remote_url -> repo_name (normalized)
    let mut url_to_repo: HashMap<String, String> = HashMap::new();
    for rd in repos {
        if let Some(url) = &rd.remote_url {
            url_to_repo.insert(normalize_git_url(url), rd.name.clone());
        }
    }

    // 3. canonical_path -> repo_name
    let mut path_to_repo: HashMap<PathBuf, String> = HashMap::new();
    for rd in repos {
        if let Ok(canonical) = rd.path.canonicalize() {
            path_to_repo.insert(canonical, rd.name.clone());
        }
    }

    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();

    for rd in repos {
        for (dep_name, ecosystem, source) in &rd.dependencies {
            let target_repo = match source {
                DepSource::Path { path } => {
                    // Resolve path dep relative to repo root
                    let resolved = rd.path.join(path);
                    resolved
                        .canonicalize()
                        .ok()
                        .and_then(|c| path_to_repo.get(&c).cloned())
                }
                DepSource::Git { url } => {
                    let normalized = normalize_git_url(url);
                    url_to_repo.get(&normalized).cloned()
                }
                DepSource::Registry => name_to_repo.get(dep_name).cloned(),
            };

            if let Some(target) = target_repo
                && target != rd.name
            {
                let key = (rd.name.clone(), target.clone(), dep_name.clone());
                if seen.insert(key) {
                    edges.push(DepEdge {
                        source_repo: rd.name.clone(),
                        target_repo: target,
                        package_name: dep_name.clone(),
                        ecosystem: ecosystem.clone(),
                    });
                }
            }
        }
    }

    // Sort by source, then target
    edges.sort_by(|a, b| {
        a.source_repo
            .cmp(&b.source_repo)
            .then_with(|| a.target_repo.cmp(&b.target_repo))
    });

    edges
}

/// Normalize a git URL to a comparable form (strip protocol, .git suffix, trailing slash).
fn normalize_git_url(url: &str) -> String {
    let url = url.trim();
    // Convert SSH to HTTPS-like form
    let url = if let Some(rest) = url.strip_prefix("git@") {
        rest.replacen(':', "/", 1)
    } else {
        url.to_string()
    };
    // Strip protocol
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("git://"))
        .unwrap_or(&url);
    // Strip .git suffix and trailing slash
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = url.strip_suffix('/').unwrap_or(url);
    url.to_lowercase()
}

fn count_active_windows(timestamps: &[u64], window_secs: u64) -> usize {
    if timestamps.is_empty() || window_secs == 0 {
        return 0;
    }
    let windows: BTreeSet<u64> = timestamps.iter().map(|t| t / window_secs).collect();
    windows.len()
}

fn compute_temporal_coupling(
    repos: &[RepoData],
    window_hours: usize,
    min_windows: usize,
) -> Vec<TemporalCouplingPair> {
    let window_secs = (window_hours as u64) * 3600;

    // Pre-compute window sets for each repo
    let repo_windows: Vec<(&str, BTreeSet<u64>)> = repos
        .iter()
        .map(|rd| {
            let windows: BTreeSet<u64> = rd
                .commit_timestamps
                .iter()
                .map(|t| t / window_secs)
                .collect();
            (rd.name.as_str(), windows)
        })
        .collect();

    let mut pairs = Vec::new();

    for i in 0..repo_windows.len() {
        for j in (i + 1)..repo_windows.len() {
            let (name_a, windows_a) = &repo_windows[i];
            let (name_b, windows_b) = &repo_windows[j];

            if windows_a.is_empty() || windows_b.is_empty() {
                continue;
            }

            let shared: usize = windows_a.intersection(windows_b).count();
            if shared < min_windows {
                continue;
            }

            let min_windows_count = windows_a.len().min(windows_b.len());
            let ratio = if min_windows_count > 0 {
                shared as f64 / min_windows_count as f64
            } else {
                0.0
            };

            pairs.push(TemporalCouplingPair {
                repo_a: name_a.to_string(),
                repo_b: name_b.to_string(),
                shared_windows: shared,
                windows_a: windows_a.len(),
                windows_b: windows_b.len(),
                coupling_ratio: ratio,
            });
        }
    }

    // Sort by coupling ratio descending
    pairs.sort_by(|a, b| {
        b.coupling_ratio
            .partial_cmp(&a.coupling_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.shared_windows.cmp(&a.shared_windows))
    });

    pairs
}
