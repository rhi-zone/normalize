//! Cross-repo command runner — discover repos and aggregate results.

use crate::output::OutputFormatter;
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Discover git repositories up to `max_depth` levels deep under `dir`.
///
/// Scans subdirectories for `.git/` directories, skipping hidden dirs.
/// Stops recursing into a directory once a `.git` is found (no nested repos).
/// Returns sorted list of repo paths.
pub fn discover_repos_depth(dir: &Path, max_depth: usize) -> Result<Vec<PathBuf>, String> {
    let mut repos = Vec::new();
    collect_repos(dir, max_depth, &mut repos)
        .map_err(|e| format!("Failed to discover repos in {}: {}", dir.display(), e))?;
    repos.sort();
    Ok(repos)
}

fn collect_repos(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if depth == 0 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_str().unwrap_or("");
        if name_str.starts_with('.') {
            continue;
        }
        if path.join(".git").is_dir() {
            repos.push(path);
        } else if depth > 1 {
            collect_repos(&path, depth - 1, repos)?;
        }
    }
    Ok(())
}

/// Discover git repositories one level deep under `dir`.
pub fn discover_repos(dir: &Path) -> Result<Vec<PathBuf>, String> {
    discover_repos_depth(dir, 1)
}

/// Outcome of running a command on a single repo.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum RepoOutcome<T: Serialize + schemars::JsonSchema> {
    Ok { data: T },
    Error { message: String },
}

/// Result for a single repo in a multi-repo run.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoResult<T: Serialize + schemars::JsonSchema> {
    pub name: String,
    pub path: PathBuf,
    #[serde(flatten)]
    pub result: RepoOutcome<T>,
}

/// Aggregated report across multiple repos.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MultiRepoReport<T: Serialize + schemars::JsonSchema> {
    pub repos: Vec<RepoResult<T>>,
}

impl<T: Serialize + schemars::JsonSchema + OutputFormatter> OutputFormatter for MultiRepoReport<T> {
    fn format_text(&self) -> String {
        if self.repos.is_empty() {
            return "No repositories found".to_string();
        }

        let mut parts = Vec::new();
        for repo in &self.repos {
            parts.push(format!("=== {} ===", repo.name));
            match &repo.result {
                RepoOutcome::Ok { data } => parts.push(data.format_text()),
                RepoOutcome::Error { message } => parts.push(format!("Error: {}", message)),
            }
            parts.push(String::new());
        }
        parts.join("\n")
    }
}

impl<T: Serialize + schemars::JsonSchema + OutputFormatter + Send> MultiRepoReport<T> {
    /// Run `f` across all repos in parallel and collect results.
    pub fn run<F>(repos: &[PathBuf], f: F) -> Self
    where
        F: Fn(&Path) -> Result<T, String> + Sync,
    {
        let results: Vec<RepoResult<T>> = repos
            .par_iter()
            .map(|repo_path| {
                let name = repo_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let result = match f(repo_path) {
                    Ok(data) => RepoOutcome::Ok { data },
                    Err(message) => RepoOutcome::Error { message },
                };

                RepoResult {
                    name,
                    path: repo_path.clone(),
                    result,
                }
            })
            .collect();

        Self { repos: results }
    }

    /// Returns true if any repo errored.
    pub fn has_errors(&self) -> bool {
        self.repos
            .iter()
            .any(|r| matches!(r.result, RepoOutcome::Error { .. }))
    }
}
