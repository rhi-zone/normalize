//! Git blame ownership analysis — bus factor, ownership concentration per file

use super::is_source_file;
use crate::output::OutputFormatter;
use glob::Pattern;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Ownership data for a file
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileOwnership {
    pub path: String,
    pub total_lines: usize,
    /// Number of distinct authors
    pub authors: usize,
    /// Top author and their percentage of lines
    pub top_author: String,
    pub top_author_pct: f64,
    /// Bus factor: number of authors needed to cover >50% of lines
    pub bus_factor: usize,
}

/// Per-repo ownership entry for multi-repo runs
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OwnershipRepoEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub files: Vec<FileOwnership>,
}

/// Ownership analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OwnershipReport {
    pub files: Vec<FileOwnership>,
    /// Per-repo results when run with --repos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<OwnershipRepoEntry>>,
}

fn format_ownership_data(files: &[FileOwnership]) -> String {
    let mut lines = Vec::new();
    lines.push("File Ownership (git blame)".to_string());
    lines.push(String::new());
    lines.push(format!(
        "{:<50} {:>6} {:>4} {:>3} {:<20}",
        "File", "Lines", "Auth", "BF", "Top Author"
    ));
    lines.push("-".repeat(90));

    for f in files {
        let display_path = truncate_path(&f.path, 48);
        let top = format!("{} ({:.0}%)", f.top_author, f.top_author_pct * 100.0);
        let top_display = if top.len() > 28 {
            format!("{}...", &top[..25])
        } else {
            top
        };
        lines.push(format!(
            "{:<50} {:>6} {:>4} {:>3} {:<20}",
            display_path, f.total_lines, f.authors, f.bus_factor, top_display
        ));
    }

    lines.push(String::new());
    lines.push("BF = Bus Factor (authors needed for >50% ownership)".to_string());
    lines.push("Low bus factor (1) means single-author risk.".to_string());

    lines.join("\n")
}

impl OutputFormatter for OwnershipReport {
    fn format_text(&self) -> String {
        if let Some(ref repos) = self.repos {
            let mut parts = Vec::new();
            for entry in repos {
                parts.push(format!("=== {} ===", entry.name));
                if let Some(ref err) = entry.error {
                    parts.push(format!("Error: {}", err));
                } else if entry.files.is_empty() {
                    parts.push("No ownership data found".to_string());
                } else {
                    parts.push(format_ownership_data(&entry.files));
                }
                parts.push(String::new());
            }
            return parts.join("\n");
        }

        if self.files.is_empty() {
            return "No ownership data found".to_string();
        }
        format_ownership_data(&self.files)
    }
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() > max_len {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    } else {
        path.to_string()
    }
}

/// Get ownership data for a single file via `git blame --line-porcelain`
fn blame_file(root: &Path, path: &str) -> Option<FileOwnership> {
    let output = std::process::Command::new("git")
        .args(["blame", "--line-porcelain", path])
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut author_lines: HashMap<String, usize> = HashMap::new();

    for line in stdout.lines() {
        if let Some(author) = line.strip_prefix("author ") {
            *author_lines.entry(author.to_string()).or_default() += 1;
        }
    }

    if author_lines.is_empty() {
        return None;
    }

    let total_lines: usize = author_lines.values().sum();

    // Sort authors by line count descending
    let mut sorted: Vec<(String, usize)> = author_lines.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let top_author = sorted[0].0.clone();
    let top_author_pct = sorted[0].1 as f64 / total_lines as f64;
    let authors = sorted.len();

    // Bus factor: minimum authors to cover >50%
    let half = total_lines / 2;
    let mut cumulative = 0;
    let mut bus_factor = 0;
    for (_, count) in &sorted {
        cumulative += count;
        bus_factor += 1;
        if cumulative > half {
            break;
        }
    }

    Some(FileOwnership {
        path: path.to_string(),
        total_lines,
        authors,
        top_author,
        top_author_pct,
        bus_factor,
    })
}

/// Collect source files from git ls-files
fn git_tracked_files(root: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success());

    match output {
        Some(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        None => Vec::new(),
    }
}

/// Analyze file ownership via git blame, returning the report.
pub fn analyze_ownership(
    root: &Path,
    limit: usize,
    exclude_patterns: &[String],
) -> Result<OwnershipReport, String> {
    let excludes: Vec<Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let git_dir = root.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    let tracked = git_tracked_files(root);
    let source_files: Vec<String> = tracked
        .into_iter()
        .filter(|path| {
            let p = Path::new(path.as_str());
            is_source_file(p) && !excludes.iter().any(|pat| pat.matches(path))
        })
        .collect();

    // Run git blame in parallel
    let mut files: Vec<FileOwnership> = source_files
        .par_iter()
        .filter_map(|path| blame_file(root, path))
        .collect();

    // Sort by bus factor ascending (riskiest first), then by top_author_pct descending
    files.sort_by(|a, b| {
        a.bus_factor
            .cmp(&b.bus_factor)
            .then_with(|| {
                b.top_author_pct
                    .partial_cmp(&a.top_author_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.total_lines.cmp(&a.total_lines))
    });
    files.truncate(limit);

    Ok(OwnershipReport { files, repos: None })
}

/// Analyze file ownership via git blame (CLI entry point)
pub fn cmd_ownership(
    root: &Path,
    limit: usize,
    exclude_patterns: &[String],
    format: &crate::output::OutputFormat,
) -> i32 {
    match analyze_ownership(root, limit, exclude_patterns) {
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
