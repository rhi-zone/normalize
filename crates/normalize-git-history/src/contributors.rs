//! Cross-repo contributor analysis — author breadth, repo bus factor, overlap.

use normalize_rank::ranked::{Column, RankEntry};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[cfg(feature = "cli")]
mod present {
    use super::ContributorsReport;
    use normalize_output::{OutputFormatter, pretty_ranked_table};
    use normalize_rank::ranked::format_ranked_table;

    fn contributors_title(report: &ContributorsReport) -> String {
        let total_authors = report.authors.len();
        let total_repos = report.repos.len();
        let total_commits: usize = report.authors.iter().map(|a| a.commits).sum();
        format!(
            "# Contributors — {} authors, {} repos, {} commits",
            total_authors, total_repos, total_commits
        )
    }

    impl OutputFormatter for ContributorsReport {
        fn format_text(&self) -> String {
            let mut out = vec![
                contributors_title(self),
                String::new(),
                format_ranked_table(
                    "## Author Summary",
                    &self.authors,
                    Some("No authors found."),
                ),
                format_ranked_table("## Repo Summary", &self.repos, Some("No repos found.")),
            ];
            if !self.overlaps.is_empty() {
                out.push(format_ranked_table(
                    "## Author Overlap",
                    &self.overlaps,
                    None,
                ));
            }
            out.join("\n\n")
        }

        fn format_pretty(&self) -> String {
            use nu_ansi_term::Style;
            let mut out = vec![
                Style::new()
                    .bold()
                    .paint(contributors_title(self))
                    .to_string(),
                String::new(),
                pretty_ranked_table(
                    "## Author Summary",
                    &self.authors,
                    Some("No authors found."),
                    |_| None,
                ),
                pretty_ranked_table(
                    "## Repo Summary",
                    &self.repos,
                    Some("No repos found."),
                    |_| None,
                ),
            ];
            if !self.overlaps.is_empty() {
                out.push(pretty_ranked_table(
                    "## Author Overlap",
                    &self.overlaps,
                    None,
                    |_| None,
                ));
            }
            out.join("\n\n")
        }
    }
}

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

impl RankEntry for ContributorInfo {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Repos"),
            Column::right("Commits"),
            Column::left("Top Repo (%)"),
            Column::left("Author"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let top = format!("{} ({:.0}%)", self.top_repo, self.top_repo_pct * 100.0);
        vec![
            self.repos.to_string(),
            self.commits.to_string(),
            top,
            self.name.clone(),
        ]
    }
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

impl RankEntry for RepoSummary {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Authors"),
            Column::right("Commits"),
            Column::right("Bus Factor"),
            Column::left("Top Author (%)"),
            Column::left("Repo"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let top = format!("{} ({:.0}%)", self.top_author, self.top_author_pct * 100.0);
        vec![
            self.authors.to_string(),
            self.commits.to_string(),
            self.bus_factor.to_string(),
            top,
            self.name.clone(),
        ]
    }
}

/// A pair of repos sharing contributors
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OverlapPair {
    pub repo_a: String,
    pub repo_b: String,
    pub shared_authors: usize,
}

impl RankEntry for OverlapPair {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Shared Authors"),
            Column::left("Repo A"),
            Column::left("Repo B"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            self.shared_authors.to_string(),
            self.repo_a.clone(),
            self.repo_b.clone(),
        ]
    }
}

/// Cross-repo contributors report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContributorsReport {
    pub authors: Vec<ContributorInfo>,
    pub repos: Vec<RepoSummary>,
    pub overlaps: Vec<OverlapPair>,
}

/// Raw shortlog entry for one repo
struct RepoShortlog {
    name: String,
    /// email -> commit count
    authors: HashMap<String, (String, usize)>, // email -> (name, commits)
}

/// Collect per-author commit counts for one repo via gix.
fn shortlog(repo: &Path) -> Option<RepoShortlog> {
    let repo_name = repo.file_name()?.to_str()?.to_string();

    let counts = normalize_git::git_author_commit_counts(repo);
    if counts.is_empty() {
        return None;
    }

    let mut authors: HashMap<String, (String, usize)> = HashMap::new();
    for entry in counts {
        let e = authors
            .entry(entry.email.clone())
            .or_insert((entry.name.clone(), 0));
        if entry.name.len() > e.0.len() {
            e.0 = entry.name;
        }
        e.1 += entry.commits;
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
    overlaps.sort_by_key(|b| std::cmp::Reverse(b.shared_authors));

    Ok(ContributorsReport {
        authors,
        repos: repo_summaries,
        overlaps,
    })
}
