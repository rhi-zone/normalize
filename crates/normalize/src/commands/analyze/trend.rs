//! Trend analysis — track health metrics over git history at regular intervals.
//!
//! Creates git worktrees at historical commits, runs health analysis on each,
//! and shows whether metrics are improving or degrading over time.

use crate::health::analyze_health;
use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

pub use crate::commands::analyze::git_history::{
    CommitInfo, format_unix_date, git_log_timestamps, run_in_worktree, select_snapshots,
};

/// Which scalar metric to track with `normalize analyze trend-metric`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrendMetric {
    /// Average cyclomatic complexity across the codebase
    Complexity,
    /// Average function length (lines) across the codebase
    Length,
    /// Overall information density score (compression + token uniqueness)
    Density,
    /// Overall test-to-implementation line ratio
    TestRatio,
}

impl FromStr for TrendMetric {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "complexity" => Ok(TrendMetric::Complexity),
            "length" => Ok(TrendMetric::Length),
            "density" => Ok(TrendMetric::Density),
            "test-ratio" | "test_ratio" => Ok(TrendMetric::TestRatio),
            other => Err(format!(
                "unknown metric '{other}'; expected one of: complexity, length, density, test-ratio"
            )),
        }
    }
}

impl fmt::Display for TrendMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrendMetric::Complexity => write!(f, "complexity"),
            TrendMetric::Length => write!(f, "length"),
            TrendMetric::Density => write!(f, "density"),
            TrendMetric::TestRatio => write!(f, "test-ratio"),
        }
    }
}

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

/// A single point in a scalar metric trend over time.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScalarTrendPoint {
    pub commit: String,
    pub date: String,
    pub timestamp: i64,
    pub value: f64,
}

/// Trend of a single scalar metric over git history.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScalarTrendReport {
    /// Metric name (e.g. "avg_complexity", "avg_length").
    pub metric: String,
    pub points: Vec<ScalarTrendPoint>,
    /// Change from first to last snapshot.
    pub delta: f64,
    pub delta_pct: f64,
    pub direction: TrendDirection,
    pub span_days: u64,
}

impl OutputFormatter for ScalarTrendReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "# {} trend ({} snapshots over {} days)",
            self.metric,
            self.points.len(),
            self.span_days,
        ));
        lines.push(String::new());

        for p in &self.points {
            lines.push(format!("{:<12} {:<9} {:.2}", p.date, p.commit, p.value));
        }

        if !self.points.is_empty() {
            let sign = if self.delta >= 0.0 { "+" } else { "" };
            lines.push(String::new());
            lines.push(format!(
                "Change: {}{:.2} ({}{:.0}%) — {}",
                sign, self.delta, sign, self.delta_pct, self.direction,
            ));
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::{Color, Style};

        let mut lines = Vec::new();
        lines.push(
            Style::new()
                .bold()
                .paint(format!(
                    "{} trend ({} snapshots over {} days)",
                    self.metric,
                    self.points.len(),
                    self.span_days,
                ))
                .to_string(),
        );
        lines.push(String::new());

        for p in &self.points {
            lines.push(format!("{:<12} {:<9} {:.2}", p.date, p.commit, p.value));
        }

        if !self.points.is_empty() {
            let sign = if self.delta >= 0.0 { "+" } else { "" };
            let (dir_str, change_str) = match self.direction {
                TrendDirection::Improving => (
                    Color::Green.paint("improving").to_string(),
                    Color::Green
                        .paint(format!(
                            "{sign}{:.2} ({sign}{:.0}%)",
                            self.delta, self.delta_pct
                        ))
                        .to_string(),
                ),
                TrendDirection::Degrading => (
                    Color::Red.paint("degrading").to_string(),
                    Color::Red
                        .paint(format!(
                            "{sign}{:.2} ({sign}{:.0}%)",
                            self.delta, self.delta_pct
                        ))
                        .to_string(),
                ),
                TrendDirection::Stable => (
                    "stable".to_string(),
                    format!("{sign}{:.2} ({sign}{:.0}%)", self.delta, self.delta_pct),
                ),
            };
            lines.push(String::new());
            lines.push(format!("Change: {} — {}", change_str, dir_str));
        }

        lines.join("\n")
    }
}

/// Run a scalar metric trend over git history.
///
/// `extract_value` takes the root of a checked-out worktree and returns the scalar.
pub fn analyze_scalar_trend<F>(
    root: &Path,
    metric: &str,
    num_snapshots: usize,
    higher_is_better: bool,
    extract_value: F,
) -> Result<ScalarTrendReport, String>
where
    F: Fn(&Path) -> Option<f64>,
{
    let commits = git_log_timestamps(root)?;
    let selected = select_snapshots(&commits, num_snapshots);

    let mut points = Vec::new();
    for commit in &selected {
        let hash = commit.hash.clone();
        let ts = commit.timestamp;
        match run_in_worktree(root, &hash, |wt| {
            extract_value(wt).ok_or_else(|| "no data".to_string())
        }) {
            Ok(value) => points.push(ScalarTrendPoint {
                commit: hash[..7.min(hash.len())].to_string(),
                date: format_unix_date(ts),
                timestamp: ts,
                value,
            }),
            Err(e) => eprintln!("Warning: skipping commit {}: {e}", &hash[..7]),
        }
    }

    if points.is_empty() {
        return Err("No data points could be collected".to_string());
    }

    let span_days = if points.len() >= 2 {
        // normalize-syntax-allow: rust/unwrap-in-impl - len >= 2 implies last() is Some
        ((points.last().unwrap().timestamp - points[0].timestamp) / 86400) as u64
    } else {
        0
    };

    let first = points[0].value;
    let last = points[points.len() - 1].value;
    let delta = last - first;
    let delta_pct = if first.abs() < 1e-9 {
        if delta.abs() < 1e-9 {
            0.0
        } else {
            100.0 * delta.signum()
        }
    } else {
        (delta / first) * 100.0
    };
    let direction = if delta.abs() < 1e-9 {
        TrendDirection::Stable
    } else if (delta > 0.0) == higher_is_better {
        TrendDirection::Improving
    } else {
        TrendDirection::Degrading
    };

    Ok(ScalarTrendReport {
        metric: metric.to_string(),
        points,
        delta,
        delta_pct,
        direction,
        span_days,
    })
}

/// Create a worktree, run health analysis, remove worktree.
fn analyze_at_commit(root: &Path, commit: &CommitInfo) -> Result<TrendSnapshot, String> {
    run_in_worktree(root, &commit.hash, |worktree_path| {
        run_health_snapshot(worktree_path, commit)
    })
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
