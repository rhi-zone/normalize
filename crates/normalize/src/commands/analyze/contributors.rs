//! Cross-repo contributor analysis — author breadth, repo bus factor, overlap

use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// Per-author aggregated info across repos
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContributorInfo {
    pub name: String,
    pub email: String,
    pub repos: usize,
    pub commits: usize,
    pub top_repo: String,
    pub top_repo_pct: f64,
}

/// Per-repo summary
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoSummary {
    pub name: String,
    pub authors: usize,
    pub commits: usize,
    pub bus_factor: usize,
    pub top_author: String,
    pub top_author_pct: f64,
}

/// A pair of repos sharing contributors
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OverlapPair {
    pub repo_a: String,
    pub repo_b: String,
    pub shared_authors: usize,
}

/// Cross-repo contributors report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContributorsReport {
    pub authors: Vec<ContributorInfo>,
    pub repos: Vec<RepoSummary>,
    pub overlaps: Vec<OverlapPair>,
}

impl OutputFormatter for ContributorsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        // Section 1: Author Summary
        lines.push("Author Summary".to_string());
        lines.push(String::new());
        lines.push(format!(
            "{:<30} {:>5} {:>8} {}",
            "Author", "Repos", "Commits", "Top Repo (%)"
        ));
        lines.push("-".repeat(70));

        for a in &self.authors {
            let top = format!("{} ({:.0}%)", a.top_repo, a.top_repo_pct * 100.0);
            lines.push(format!(
                "{:<30} {:>5} {:>8} {}",
                truncate(&a.name, 28),
                a.repos,
                a.commits,
                top,
            ));
        }

        // Section 2: Repo Summary
        lines.push(String::new());
        lines.push("Repo Summary".to_string());
        lines.push(String::new());
        lines.push(format!(
            "{:<25} {:>7} {:>8} {:>3} {}",
            "Repo", "Authors", "Commits", "BF", "Top Author (%)"
        ));
        lines.push("-".repeat(75));

        for r in &self.repos {
            let top = format!("{} ({:.0}%)", r.top_author, r.top_author_pct * 100.0);
            lines.push(format!(
                "{:<25} {:>7} {:>8} {:>3} {}",
                truncate(&r.name, 23),
                r.authors,
                r.commits,
                r.bus_factor,
                top,
            ));
        }

        // Section 3: Author Overlap
        if !self.overlaps.is_empty() {
            lines.push(String::new());
            lines.push("Author Overlap".to_string());
            lines.push(String::new());
            lines.push(format!(
                "{:<25} {:<25} {:>14}",
                "Repo A", "Repo B", "Shared Authors"
            ));
            lines.push("-".repeat(66));

            for o in &self.overlaps {
                lines.push(format!(
                    "{:<25} {:<25} {:>14}",
                    truncate(&o.repo_a, 23),
                    truncate(&o.repo_b, 23),
                    o.shared_authors,
                ));
            }
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

/// Raw shortlog entry for one repo
struct RepoShortlog {
    name: String,
    /// email -> commit count
    authors: HashMap<String, (String, usize)>, // email -> (name, commits)
}

/// Run `git shortlog -sne --all` on a single repo
fn shortlog(repo: &Path) -> Option<RepoShortlog> {
    let repo_name = repo.file_name()?.to_str()?.to_string();

    let output = std::process::Command::new("git")
        .args(["shortlog", "-sne", "--all"])
        .current_dir(repo)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut authors: HashMap<String, (String, usize)> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "   342\tAlice <alice@example.com>"
        let (count_str, rest) = line.split_once('\t')?;
        let count: usize = count_str.trim().parse().ok()?;

        // Parse "Name <email>"
        let (name, email) = if let Some(angle_start) = rest.rfind('<') {
            let name = rest[..angle_start].trim().to_string();
            let email = rest[angle_start + 1..]
                .trim_end_matches('>')
                .trim()
                .to_string();
            (name, email)
        } else {
            (rest.trim().to_string(), String::new())
        };

        let entry = authors.entry(email.clone()).or_insert((name, 0));
        entry.1 += count;
    }

    Some(RepoShortlog {
        name: repo_name,
        authors,
    })
}

/// Analyze contributors across multiple repos.
pub fn analyze_contributors(repos: &[std::path::PathBuf]) -> Result<ContributorsReport, String> {
    use rayon::prelude::*;

    let shortlogs: Vec<RepoShortlog> = repos.par_iter().filter_map(|r| shortlog(r)).collect();

    if shortlogs.is_empty() {
        return Err("No git repositories with commit history found".to_string());
    }

    // Aggregate by email across repos
    // email -> (best_name, { repo_name -> commits })
    let mut author_map: HashMap<String, (String, BTreeMap<String, usize>)> = HashMap::new();

    for sl in &shortlogs {
        for (email, (name, commits)) in &sl.authors {
            let entry = author_map
                .entry(email.clone())
                .or_insert_with(|| (name.clone(), BTreeMap::new()));
            // Keep the longest name variant (heuristic for most complete)
            if name.len() > entry.0.len() {
                entry.0 = name.clone();
            }
            *entry.1.entry(sl.name.clone()).or_default() += commits;
        }
    }

    // Build author summary
    let mut authors: Vec<ContributorInfo> = author_map
        .iter()
        .map(|(email, (name, repo_commits))| {
            let total_commits: usize = repo_commits.values().sum();
            let (top_repo, &top_commits) = repo_commits
                .iter()
                .max_by_key(|(_, c)| *c)
                .expect("non-empty");
            ContributorInfo {
                name: name.clone(),
                email: email.clone(),
                repos: repo_commits.len(),
                commits: total_commits,
                top_repo: top_repo.clone(),
                top_repo_pct: top_commits as f64 / total_commits as f64,
            }
        })
        .collect();

    // Sort by repo breadth descending, then commits descending
    authors.sort_by(|a, b| {
        b.repos
            .cmp(&a.repos)
            .then_with(|| b.commits.cmp(&a.commits))
    });

    // Build repo summary
    // repo_name -> set of emails
    let mut repo_authors_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for sl in &shortlogs {
        let emails: BTreeSet<String> = sl.authors.keys().cloned().collect();
        repo_authors_map.insert(sl.name.clone(), emails);
    }

    let mut repo_summaries: Vec<RepoSummary> = shortlogs
        .iter()
        .map(|sl| {
            let total_commits: usize = sl.authors.values().map(|(_, c)| c).sum();
            let author_count = sl.authors.len();

            // Find top author
            let (top_email, _) = sl
                .authors
                .iter()
                .max_by_key(|(_, (_, c))| *c)
                .expect("non-empty");
            let (top_name, top_commits) = &sl.authors[top_email];

            // Bus factor: sort by commits desc, accumulate until >50%
            let mut sorted: Vec<usize> = sl.authors.values().map(|(_, c)| *c).collect();
            sorted.sort_unstable_by(|a, b| b.cmp(a));
            let half = total_commits / 2;
            let mut cumulative = 0;
            let mut bus_factor = 0;
            for c in &sorted {
                cumulative += c;
                bus_factor += 1;
                if cumulative > half {
                    break;
                }
            }

            RepoSummary {
                name: sl.name.clone(),
                authors: author_count,
                commits: total_commits,
                bus_factor,
                top_author: top_name.clone(),
                top_author_pct: *top_commits as f64 / total_commits as f64,
            }
        })
        .collect();

    // Sort by bus factor ascending (riskiest first)
    repo_summaries.sort_by(|a, b| {
        a.bus_factor
            .cmp(&b.bus_factor)
            .then_with(|| a.authors.cmp(&b.authors))
    });

    // Build overlap pairs
    let repo_names: Vec<&String> = repo_authors_map.keys().collect();
    let mut overlaps = Vec::new();

    for i in 0..repo_names.len() {
        for j in (i + 1)..repo_names.len() {
            let a = repo_names[i];
            let b = repo_names[j];
            let shared = repo_authors_map[a]
                .intersection(&repo_authors_map[b])
                .count();
            if shared > 0 {
                overlaps.push(OverlapPair {
                    repo_a: a.clone(),
                    repo_b: b.clone(),
                    shared_authors: shared,
                });
            }
        }
    }

    // Sort by shared count descending
    overlaps.sort_by(|a, b| b.shared_authors.cmp(&a.shared_authors));

    Ok(ContributorsReport {
        authors,
        repos: repo_summaries,
        overlaps,
    })
}
