//! Cross-repo activity over time — commit volume, author focus, and churn trends

use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Trend direction based on recent vs older window comparison.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub enum Trend {
    Rising,
    Stable,
    Declining,
}

impl std::fmt::Display for Trend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trend::Rising => write!(f, "Rising"),
            Trend::Stable => write!(f, "Stable"),
            Trend::Declining => write!(f, "Declining"),
        }
    }
}

/// Per-repo activity in a single time window.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WindowActivity {
    pub window: String,
    pub commits: usize,
    pub authors: usize,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Per-repo activity summary.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoActivity {
    pub name: String,
    pub total_commits: usize,
    pub total_authors: usize,
    pub windows: Vec<WindowActivity>,
    pub trend: Trend,
}

/// Top-level activity report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ActivityReport {
    pub repos: Vec<RepoActivity>,
    pub window_size: String,
    pub window_count: usize,
}

impl OutputFormatter for ActivityReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        // Section 1: Repo Overview
        lines.push("Repo Overview".to_string());
        lines.push(format!(
            "  {:<24} {:>8} {:>8} {:>10}",
            "Repo", "Commits", "Authors", "Trend",
        ));
        lines.push(format!("  {}", "-".repeat(54)));

        // Sort: declining first, then stable, then rising
        let mut sorted: Vec<&RepoActivity> = self.repos.iter().collect();
        sorted.sort_by_key(|r| match r.trend {
            Trend::Declining => 0,
            Trend::Stable => 1,
            Trend::Rising => 2,
        });

        for repo in &sorted {
            let trend_icon = match repo.trend {
                Trend::Declining => "\u{25bc} Declining",
                Trend::Stable => "\u{2500} Stable",
                Trend::Rising => "\u{25b2} Rising",
            };
            lines.push(format!(
                "  {:<24} {:>8} {:>8} {}",
                truncate(&repo.name, 22),
                repo.total_commits,
                repo.total_authors,
                trend_icon,
            ));
        }

        // Section 2: Timeline sparklines
        lines.push(String::new());
        lines.push(format!(
            "Timeline ({}ly, last {} windows)",
            self.window_size, self.window_count,
        ));
        lines.push(String::new());

        let max_name_len = self
            .repos
            .iter()
            .map(|r| r.name.len().min(16))
            .max()
            .unwrap_or(8);

        for repo in &sorted {
            let counts: Vec<usize> = repo.windows.iter().map(|w| w.commits).collect();
            let spark = sparkline(&counts);
            lines.push(format!(
                "  {:<width$} {} {:>4} commits",
                truncate(&repo.name, max_name_len),
                spark,
                repo.total_commits,
                width = max_name_len,
            ));
        }

        // Section 3: Author Focus (last 3 windows)
        lines.push(String::new());
        let focus_windows = 3.min(self.window_count);
        lines.push(format!(
            "Recent Author Focus (last {} {}s)",
            focus_windows, self.window_size,
        ));
        lines.push(String::new());

        for repo in &sorted {
            let recent_start = repo.windows.len().saturating_sub(focus_windows);
            let recent = &repo.windows[recent_start..];

            // Count commits per author from raw data isn't available in WindowActivity,
            // so we show the author count trend instead
            let recent_authors: usize = recent.iter().map(|w| w.authors).max().unwrap_or(0);
            let recent_commits: usize = recent.iter().map(|w| w.commits).sum();

            if recent_commits > 0 {
                lines.push(format!(
                    "  {}: {} commits by up to {} authors",
                    truncate(&repo.name, 22),
                    recent_commits,
                    recent_authors,
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

const SPARK_CHARS: &[char] = &[
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}',
];

fn sparkline(values: &[usize]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let max = *values.iter().max().unwrap_or(&1);
    if max == 0 {
        return values.iter().map(|_| SPARK_CHARS[0]).collect();
    }
    values
        .iter()
        .map(|&v| {
            let idx = ((v as f64 / max as f64) * 7.0).round() as usize;
            SPARK_CHARS[idx.min(7)]
        })
        .collect()
}

/// Analyze activity across repos.
pub fn analyze_activity(
    repos: &[PathBuf],
    window_size: &str,
    window_count: usize,
) -> Result<ActivityReport, String> {
    use rayon::prelude::*;

    if repos.is_empty() {
        return Err("No repositories provided".to_string());
    }

    let repo_data: Vec<RawRepoData> = repos
        .par_iter()
        .filter_map(|r| gather_repo_data(r))
        .collect();

    if repo_data.is_empty() {
        return Err("No analyzable repositories found".to_string());
    }

    // Determine window labels (most recent N windows)
    let all_windows = generate_window_labels(window_size, window_count);

    let repos_activity: Vec<RepoActivity> = repo_data
        .into_iter()
        .map(|rd| build_repo_activity(rd, window_size, &all_windows))
        .collect();

    Ok(ActivityReport {
        repos: repos_activity,
        window_size: window_size.to_string(),
        window_count,
    })
}

// ============================================================================
// Internal types and helpers
// ============================================================================

struct CommitInfo {
    timestamp: u64,
    author: String,
    file_stats: Vec<FileStat>,
}

struct FileStat {
    insertions: usize,
    deletions: usize,
}

struct RawRepoData {
    name: String,
    commits: Vec<CommitInfo>,
}

fn gather_repo_data(repo: &Path) -> Option<RawRepoData> {
    let name = repo.file_name()?.to_str()?.to_string();

    let output = std::process::Command::new("git")
        .args(["log", "--all", "--format=%H %at %ae", "--numstat"])
        .current_dir(repo)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits = parse_git_log(&stdout);

    Some(RawRepoData { name, commits })
}

fn parse_git_log(output: &str) -> Vec<CommitInfo> {
    let mut commits = Vec::new();
    let mut current: Option<CommitInfo> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse as commit header: <hash> <timestamp> <email>
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() == 3
            && parts[0].len() == 40
            && parts[0].chars().all(|c| c.is_ascii_hexdigit())
        {
            // Save previous commit
            if let Some(c) = current.take() {
                commits.push(c);
            }
            let timestamp = parts[1].parse::<u64>().unwrap_or(0);
            current = Some(CommitInfo {
                timestamp,
                author: parts[2].to_string(),
                file_stats: Vec::new(),
            });
        } else if let Some(ref mut c) = current {
            // Try to parse as numstat line: <ins>\t<del>\t<file>
            let stat_parts: Vec<&str> = line.split('\t').collect();
            if stat_parts.len() >= 3 {
                let ins = stat_parts[0].parse::<usize>().unwrap_or(0);
                let del = stat_parts[1].parse::<usize>().unwrap_or(0);
                c.file_stats.push(FileStat {
                    insertions: ins,
                    deletions: del,
                });
            }
        }
    }

    if let Some(c) = current {
        commits.push(c);
    }

    commits
}

/// Generate the last N window labels for the given granularity.
fn generate_window_labels(window_size: &str, count: usize) -> Vec<String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match window_size {
        "week" => {
            // ISO week: days since epoch / 7
            let current_day = now / 86400;
            // Find Monday of current week (epoch was Thursday, so day 0 = Thursday)
            // We'll just use week numbers from epoch for simplicity
            let current_week = current_day / 7;
            (0..count)
                .rev()
                .map(|i| {
                    let week = current_week - i as u64;
                    let start_day = week * 7;
                    let start_ts = start_day * 86400;
                    // Format as YYYY-Www
                    let (year, month, day) = timestamp_to_ymd(start_ts);
                    let _ = (month, day);
                    format!("{}-W{:02}", year, (start_day % 365) / 7 + 1)
                })
                .collect()
        }
        _ => {
            // "month" (default)
            let (year, month, _) = timestamp_to_ymd(now);
            let mut labels = Vec::with_capacity(count);
            let mut y = year as i32;
            let mut m = month as i32;

            for _ in 0..count {
                labels.push(format!("{:04}-{:02}", y, m));
                m -= 1;
                if m < 1 {
                    m = 12;
                    y -= 1;
                }
            }
            labels.reverse();
            labels
        }
    }
}

fn timestamp_to_ymd(ts: u64) -> (u32, u32, u32) {
    // Simple conversion from unix timestamp to (year, month, day)
    let days = (ts / 86400) as i64;
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m, d)
}

fn timestamp_to_window_key(ts: u64, window_size: &str) -> String {
    match window_size {
        "week" => {
            let day = ts / 86400;
            let week = day / 7;
            let start_day = week * 7;
            let start_ts = start_day * 86400;
            let (year, _, _) = timestamp_to_ymd(start_ts);
            format!("{}-W{:02}", year, (start_day % 365) / 7 + 1)
        }
        _ => {
            let (year, month, _) = timestamp_to_ymd(ts);
            format!("{:04}-{:02}", year, month)
        }
    }
}

fn build_repo_activity(
    rd: RawRepoData,
    window_size: &str,
    window_labels: &[String],
) -> RepoActivity {
    // Bucket commits into windows
    let mut window_map: BTreeMap<String, WindowBucket> = BTreeMap::new();
    let mut all_authors = HashSet::new();

    for commit in &rd.commits {
        let key = timestamp_to_window_key(commit.timestamp, window_size);
        let bucket = window_map.entry(key).or_default();
        bucket.commits += 1;
        bucket.authors.insert(commit.author.clone());
        all_authors.insert(commit.author.clone());
        for fs in &commit.file_stats {
            bucket.files_changed += 1;
            bucket.insertions += fs.insertions;
            bucket.deletions += fs.deletions;
        }
    }

    // Build ordered windows based on labels
    let windows: Vec<WindowActivity> = window_labels
        .iter()
        .map(|label| {
            if let Some(bucket) = window_map.get(label) {
                WindowActivity {
                    window: label.clone(),
                    commits: bucket.commits,
                    authors: bucket.authors.len(),
                    files_changed: bucket.files_changed,
                    insertions: bucket.insertions,
                    deletions: bucket.deletions,
                }
            } else {
                WindowActivity {
                    window: label.clone(),
                    commits: 0,
                    authors: 0,
                    files_changed: 0,
                    insertions: 0,
                    deletions: 0,
                }
            }
        })
        .collect();

    let trend = compute_trend(&windows);
    let total_commits: usize = windows.iter().map(|w| w.commits).sum();

    RepoActivity {
        name: rd.name,
        total_commits,
        total_authors: all_authors.len(),
        windows,
        trend,
    }
}

#[derive(Default)]
struct WindowBucket {
    commits: usize,
    authors: HashSet<String>,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
}

fn compute_trend(windows: &[WindowActivity]) -> Trend {
    if windows.len() < 2 {
        return Trend::Stable;
    }

    let mid = windows.len() / 2;
    let first_half: usize = windows[..mid].iter().map(|w| w.commits).sum();
    let second_half: usize = windows[mid..].iter().map(|w| w.commits).sum();

    if first_half == 0 && second_half == 0 {
        return Trend::Stable;
    }
    if first_half == 0 {
        return Trend::Rising;
    }

    let ratio = second_half as f64 / first_half as f64;
    if ratio > 1.2 {
        Trend::Rising
    } else if ratio < 0.8 {
        Trend::Declining
    } else {
        Trend::Stable
    }
}
