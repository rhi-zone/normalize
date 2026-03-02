//! Trend analysis — track health metrics over git history at regular intervals.
//!
//! Creates git worktrees at historical commits, runs health analysis on each,
//! and shows whether metrics are improving or degrading over time.

use crate::health::analyze_health;
use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::process::Command;

/// Direction of a metric's change over time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

impl fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrendDirection::Improving => write!(f, "improving"),
            TrendDirection::Stable => write!(f, "stable"),
            TrendDirection::Degrading => write!(f, "degrading"),
        }
    }
}

/// A single point-in-time health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TrendSnapshot {
    pub commit: String,
    pub timestamp: i64,
    pub date: String,
    pub health_score: f64,
    pub grade: String,
    pub total_files: usize,
    pub total_lines: usize,
    pub avg_complexity: f64,
    pub test_ratio: Option<f64>,
    pub uniqueness_ratio: Option<f64>,
    pub ceremony_ratio: Option<f64>,
    pub density_score: Option<f64>,
}

/// Change in a metric between first and last snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricDelta {
    pub name: String,
    pub first: f64,
    pub last: f64,
    pub change: f64,
    pub change_pct: f64,
    pub direction: TrendDirection,
}

/// Full trend report across historical snapshots.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TrendReport {
    pub snapshots: Vec<TrendSnapshot>,
    pub deltas: Vec<MetricDelta>,
    pub span_days: u64,
    pub num_snapshots: usize,
}

struct CommitInfo {
    hash: String,
    timestamp: i64,
}

/// Get all commits with timestamps, oldest first.
fn git_log_timestamps(root: &Path) -> Result<Vec<CommitInfo>, String> {
    let output = Command::new("git")
        .args(["log", "--format=%H%x00%at", "--reverse"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git log: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git log failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<CommitInfo> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\0').collect();
            if parts.len() == 2 {
                let ts = parts[1].parse::<i64>().ok()?;
                Some(CommitInfo {
                    hash: parts[0].to_string(),
                    timestamp: ts,
                })
            } else {
                None
            }
        })
        .collect();

    if commits.is_empty() {
        return Err("No commits found in git history".to_string());
    }

    Ok(commits)
}

/// Pick N commits at regular time intervals from the commit list.
fn select_snapshots(commits: &[CommitInfo], n: usize) -> Vec<&CommitInfo> {
    if commits.len() <= n {
        return commits.iter().collect();
    }

    let first_ts = commits[0].timestamp;
    let last_ts = commits[commits.len() - 1].timestamp;

    if first_ts == last_ts {
        // All commits at same timestamp — just pick evenly spaced indices
        let step = commits.len() / n;
        let mut selected: Vec<&CommitInfo> = (0..n).map(|i| &commits[i * step]).collect();
        // Always include last
        if let Some(last) = commits.last()
            && selected
                .last()
                .is_none_or(|prev: &&CommitInfo| prev.hash != last.hash)
        {
            selected.pop();
            selected.push(last);
        }
        return selected;
    }

    // Place n evenly-spaced targets from first_ts to last_ts (inclusive of both endpoints).
    let interval = (last_ts - first_ts) as f64 / (n - 1).max(1) as f64;
    let mut selected = Vec::with_capacity(n);

    for i in 0..n {
        let target_ts = first_ts as f64 + interval * i as f64;
        // Find the commit closest to the target timestamp
        let best = commits
            .iter()
            .min_by_key(|c| ((c.timestamp as f64) - target_ts).abs() as i64);
        if let Some(commit) = best {
            // Avoid duplicates
            if selected
                .last()
                .is_none_or(|prev: &&CommitInfo| prev.hash != commit.hash)
            {
                selected.push(commit);
            }
        }
    }

    selected
}

/// Create a worktree, run health analysis, remove worktree.
fn analyze_at_commit(root: &Path, commit: &CommitInfo) -> Result<TrendSnapshot, String> {
    let short_hash = &commit.hash[..7.min(commit.hash.len())];
    let worktree_name = format!("normalize-trend-{short_hash}");
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    // Clean up any stale worktree at this path
    if worktree_path.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
    }

    // Create worktree
    let add_output = Command::new("git")
        .args(["worktree", "add", &worktree_str, &commit.hash, "--detach"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to create worktree: {e}"))?;

    if !add_output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_output.stderr).trim()
        ));
    }

    // Run health analysis (always clean up afterward)
    let result = run_health_snapshot(&worktree_path, commit);

    // Remove worktree
    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

    result
}

fn run_health_snapshot(worktree_path: &Path, commit: &CommitInfo) -> Result<TrendSnapshot, String> {
    let health = analyze_health(worktree_path);
    let score = health.calculate_health_score();
    let grade = health.grade().to_string();
    let date = format_unix_date(commit.timestamp);

    Ok(TrendSnapshot {
        commit: commit.hash[..7.min(commit.hash.len())].to_string(),
        timestamp: commit.timestamp,
        date,
        health_score: score,
        grade,
        total_files: health.total_files,
        total_lines: health.total_lines,
        avg_complexity: health.avg_complexity,
        test_ratio: health.test_ratio,
        uniqueness_ratio: health.uniqueness_ratio,
        ceremony_ratio: health.ceremony_ratio,
        density_score: health.density_score,
    })
}

fn format_unix_date(ts: i64) -> String {
    // Manual conversion from unix timestamp to YYYY-MM-DD
    // Using chrono-free approach: shell out to date or compute manually
    let output = Command::new("date")
        .args(["-d", &format!("@{ts}"), "+%Y-%m-%d"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => format!("{ts}"),
    }
}

fn compute_deltas(snapshots: &[TrendSnapshot]) -> Vec<MetricDelta> {
    if snapshots.len() < 2 {
        return Vec::new();
    }

    let first = &snapshots[0];
    let last = &snapshots[snapshots.len() - 1];

    let mut deltas = vec![
        make_delta("health_score", first.health_score, last.health_score, true),
        make_delta(
            "avg_complexity",
            first.avg_complexity,
            last.avg_complexity,
            false,
        ),
        make_delta(
            "total_files",
            first.total_files as f64,
            last.total_files as f64,
            true,
        ),
        make_delta(
            "total_lines",
            first.total_lines as f64,
            last.total_lines as f64,
            true,
        ),
    ];

    if let (Some(f), Some(l)) = (first.test_ratio, last.test_ratio) {
        deltas.push(make_delta("test_ratio", f, l, true));
    }

    if let (Some(f), Some(l)) = (first.uniqueness_ratio, last.uniqueness_ratio) {
        deltas.push(make_delta("uniqueness_ratio", f, l, true));
    }

    if let (Some(f), Some(l)) = (first.ceremony_ratio, last.ceremony_ratio) {
        deltas.push(make_delta("ceremony_ratio", f, l, false));
    }

    if let (Some(f), Some(l)) = (first.density_score, last.density_score) {
        deltas.push(make_delta("density_score", f, l, true));
    }

    deltas
}

fn make_delta(name: &str, first: f64, last: f64, higher_is_better: bool) -> MetricDelta {
    let change = last - first;
    let change_pct = if first.abs() < 1e-9 {
        if change.abs() < 1e-9 {
            0.0
        } else {
            100.0 * change.signum()
        }
    } else {
        (change / first) * 100.0
    };

    let direction = if change.abs() < 1e-9 {
        TrendDirection::Stable
    } else if (change > 0.0) == higher_is_better {
        TrendDirection::Improving
    } else {
        TrendDirection::Degrading
    };

    MetricDelta {
        name: name.to_string(),
        first,
        last,
        change,
        change_pct,
        direction,
    }
}

/// Run trend analysis over git history.
pub fn analyze_trend(root: &Path, num_snapshots: usize) -> Result<TrendReport, String> {
    let commits = git_log_timestamps(root)?;
    let selected = select_snapshots(&commits, num_snapshots);

    let mut snapshots = Vec::new();
    for commit in &selected {
        match analyze_at_commit(root, commit) {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(e) => {
                eprintln!("Warning: skipping commit {}: {e}", &commit.hash[..7]);
            }
        }
    }

    if snapshots.is_empty() {
        return Err("No snapshots could be collected".to_string());
    }

    let span_days = if snapshots.len() >= 2 {
        let first_ts = snapshots[0].timestamp;
        let last_ts = snapshots[snapshots.len() - 1].timestamp;
        ((last_ts - first_ts) / 86400) as u64
    } else {
        0
    };

    let deltas = compute_deltas(&snapshots);
    let num = snapshots.len();

    Ok(TrendReport {
        snapshots,
        deltas,
        span_days,
        num_snapshots: num,
    })
}

fn format_ratio(v: Option<f64>) -> String {
    match v {
        Some(r) => format!("{:>5.0}%", r * 100.0),
        None => "    -".to_string(),
    }
}

fn format_score_pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

impl OutputFormatter for TrendReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "# Health Trend ({} snapshots over {} days)",
            self.num_snapshots, self.span_days
        ));
        lines.push(String::new());

        // Header
        lines.push(format!(
            "{:<12} {:<9} {:>6} {:>6} {:>6} {:>8} {:>11} {:>7} {:>7}",
            "Date", "Commit", "Score", "Grade", "Files", "Lines", "Complexity", "Tests", "Unique"
        ));

        for snap in &self.snapshots {
            lines.push(format!(
                "{:<12} {:<9} {:>5} {:>6} {:>6} {:>8} {:>11.1} {} {}",
                snap.date,
                snap.commit,
                format_score_pct(snap.health_score),
                snap.grade,
                snap.total_files,
                snap.total_lines,
                snap.avg_complexity,
                format_ratio(snap.test_ratio),
                format_ratio(snap.uniqueness_ratio),
            ));
        }

        if !self.deltas.is_empty() {
            lines.push(String::new());
            lines.push("Trends:".to_string());
            for d in &self.deltas {
                let first_str = format_metric_value(&d.name, d.first);
                let last_str = format_metric_value(&d.name, d.last);
                let sign = if d.change_pct >= 0.0 { "+" } else { "" };
                lines.push(format!(
                    "  {:<18} {} → {}  {}{:.0}%  {}",
                    d.name, first_str, last_str, sign, d.change_pct, d.direction
                ));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "\x1b[1m# Health Trend ({} snapshots over {} days)\x1b[0m",
            self.num_snapshots, self.span_days
        ));
        lines.push(String::new());

        // Header
        lines.push(format!(
            "\x1b[1m{:<12} {:<9} {:>6} {:>6} {:>6} {:>8} {:>11} {:>7} {:>7}\x1b[0m",
            "Date", "Commit", "Score", "Grade", "Files", "Lines", "Complexity", "Tests", "Unique"
        ));

        for snap in &self.snapshots {
            let grade_colored = color_grade(&snap.grade);
            lines.push(format!(
                "{:<12} {:<9} {:>5} {:>6} {:>6} {:>8} {:>11.1} {} {}",
                snap.date,
                snap.commit,
                format_score_pct(snap.health_score),
                grade_colored,
                snap.total_files,
                snap.total_lines,
                snap.avg_complexity,
                format_ratio(snap.test_ratio),
                format_ratio(snap.uniqueness_ratio),
            ));
        }

        if !self.deltas.is_empty() {
            lines.push(String::new());
            lines.push("\x1b[1mTrends:\x1b[0m".to_string());
            for d in &self.deltas {
                let first_str = format_metric_value(&d.name, d.first);
                let last_str = format_metric_value(&d.name, d.last);
                let sign = if d.change_pct >= 0.0 { "+" } else { "" };
                let dir_colored = match d.direction {
                    TrendDirection::Improving => "\x1b[32mimproving\x1b[0m",
                    TrendDirection::Degrading => "\x1b[31mdegrading\x1b[0m",
                    TrendDirection::Stable => "stable",
                };
                let change_colored = match d.direction {
                    TrendDirection::Improving => {
                        format!("\x1b[32m{sign}{:.0}%\x1b[0m", d.change_pct)
                    }
                    TrendDirection::Degrading => {
                        format!("\x1b[31m{sign}{:.0}%\x1b[0m", d.change_pct)
                    }
                    TrendDirection::Stable => format!("{sign}{:.0}%", d.change_pct),
                };
                lines.push(format!(
                    "  {:<18} {} → {}  {}  {}",
                    d.name, first_str, last_str, change_colored, dir_colored
                ));
            }
        }

        lines.join("\n")
    }
}

fn format_metric_value(name: &str, value: f64) -> String {
    match name {
        "health_score" | "test_ratio" | "uniqueness_ratio" | "ceremony_ratio" | "density_score" => {
            format!("{:.0}%", value * 100.0)
        }
        "avg_complexity" => format!("{value:.1}"),
        _ => format!("{value:.0}"),
    }
}

fn color_grade(grade: &str) -> String {
    match grade {
        "A" => "\x1b[32mA\x1b[0m".to_string(),
        "B" => "\x1b[32mB\x1b[0m".to_string(),
        "C" => "\x1b[33mC\x1b[0m".to_string(),
        "D" => "\x1b[31mD\x1b[0m".to_string(),
        "F" => "\x1b[31mF\x1b[0m".to_string(),
        _ => grade.to_string(),
    }
}
