use crate::output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StaleSummary {
    dir: String,
    commits_since_update: usize,
    last_summary_commit: String,
    /// True if the directory has uncommitted changes not reflected in SUMMARY.md.
    has_uncommitted_changes: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct MissingSummary {
    dir: String,
    total_commits: usize,
    /// True if the directory has uncommitted changes with no SUMMARY.md at all.
    has_uncommitted_changes: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StaleSummaryReport {
    stale: Vec<StaleSummary>,
    missing: Vec<MissingSummary>,
    dirs_checked: usize,
    threshold: usize,
}

impl OutputFormatter for StaleSummaryReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("SUMMARY.md Freshness Check".to_string());
        lines.push(String::new());
        lines.push(format!("Directories checked: {}", self.dirs_checked));
        lines.push(format!("Staleness threshold: {} commits", self.threshold));
        lines.push(String::new());

        let total = self.stale.len() + self.missing.len();
        if total == 0 {
            lines.push("All SUMMARY.md files are up to date.".to_string());
        } else {
            if !self.missing.is_empty() {
                lines.push(format!("Missing SUMMARY.md ({}):", self.missing.len()));
                for m in &self.missing {
                    let suffix = if m.has_uncommitted_changes {
                        " + uncommitted changes".to_string()
                    } else {
                        format!("{} commits with no SUMMARY.md", m.total_commits)
                    };
                    lines.push(format!("  {} ({})", m.dir, suffix));
                }
                lines.push(String::new());
            }
            if !self.stale.is_empty() {
                lines.push(format!("Stale SUMMARY.md ({}):", self.stale.len()));
                for s in &self.stale {
                    let suffix = if s.has_uncommitted_changes {
                        format!(
                            "{} commits + uncommitted changes since last update",
                            s.commits_since_update
                        )
                    } else {
                        format!("{} commits since last update", s.commits_since_update)
                    };
                    lines.push(format!("  {} ({})", s.dir, suffix));
                }
            }
        }

        lines.join("\n")
    }
}

fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | "node_modules" | ".git" | ".claude" | "dist" | "build" | "__pycache__"
    ) || name.starts_with('.')
}

/// Returns the commit hash of the last commit touching `rel_path`, or None.
fn git_last_commit(root: &Path, rel_path: &str) -> Option<String> {
    let out = Command::new("git")
        .args([
            "-C",
            root.to_str().unwrap_or("."),
            "log",
            "-1",
            "--format=%H",
            "--",
            rel_path,
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Counts commits touching `rel_dir` since `since_commit` (exclusive).
/// If `since_commit` is None, counts all commits touching `rel_dir`.
fn git_commit_count(root: &Path, since_commit: Option<&str>, rel_dir: &str) -> usize {
    let range = since_commit
        .map(|h| format!("{}..HEAD", h))
        .unwrap_or_else(|| "HEAD".into());
    let out = Command::new("git")
        .args([
            "-C",
            root.to_str().unwrap_or("."),
            "log",
            "--oneline",
            &range,
            "--",
            rel_dir,
        ])
        .output()
        .ok();
    let stdout = out.as_ref().map(|o| o.stdout.as_slice()).unwrap_or(&[]);
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Returns true if `rel_dir` has uncommitted changes (staged or unstaged) that are
/// NOT limited to SUMMARY.md itself (i.e., real content changes needing documentation).
fn git_has_uncommitted_content_changes(root: &Path, rel_dir: &str) -> bool {
    let dir_prefix = if rel_dir == "." {
        String::new()
    } else {
        format!("{}/", rel_dir)
    };
    let out = Command::new("git")
        .args([
            "-C",
            root.to_str().unwrap_or("."),
            "status",
            "--short",
            "--",
            rel_dir,
        ])
        .output();
    let Ok(out) = out else { return false };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .any(|l| {
            // Each line: "XY path" where XY are status codes
            let path = l.get(3..).unwrap_or("").trim();
            // Exclude SUMMARY.md itself from the "content changed" signal
            path != format!("{}SUMMARY.md", dir_prefix).as_str() && path != "SUMMARY.md"
        })
}

/// Returns true if SUMMARY.md for `rel_dir` has uncommitted changes (it's being updated).
fn git_summary_has_uncommitted_changes(root: &Path, summary_path: &str) -> bool {
    let out = Command::new("git")
        .args([
            "-C",
            root.to_str().unwrap_or("."),
            "status",
            "--short",
            "--",
            summary_path,
        ])
        .output();
    let Ok(out) = out else { return false };
    !String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

pub fn build_stale_summary_report(root: &Path, threshold: usize) -> StaleSummaryReport {
    let mut stale = Vec::new();
    let mut missing = Vec::new();
    let mut dirs_checked = 0;

    let dirs: Vec<_> = walkdir::WalkDir::new(root)
        .min_depth(0)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .filter(|e| {
            !e.path()
                .components()
                .any(|c| is_excluded_dir(c.as_os_str().to_string_lossy().as_ref()))
        })
        .collect();

    for entry in &dirs {
        let dir_path = entry.path();
        let rel_dir = dir_path
            .strip_prefix(root)
            .unwrap_or(dir_path)
            .to_string_lossy();
        let rel_dir_git = if rel_dir.is_empty() {
            ".".to_string()
        } else {
            rel_dir.to_string()
        };

        let has_files = std::fs::read_dir(dir_path)
            .map(|mut rd| {
                rd.any(|e| {
                    e.map(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if !has_files {
            continue;
        }

        dirs_checked += 1;

        let summary_path = if rel_dir.is_empty() {
            "SUMMARY.md".to_string()
        } else {
            format!("{}/SUMMARY.md", rel_dir)
        };

        let dir_label = if rel_dir.is_empty() {
            ".".to_string()
        } else {
            rel_dir.to_string()
        };

        // Check for uncommitted content changes in this directory
        let content_dirty = git_has_uncommitted_content_changes(root, &rel_dir_git);
        let summary_dirty = git_summary_has_uncommitted_changes(root, &summary_path);

        // If content is dirty but SUMMARY.md is already being updated, no issue
        let has_uncommitted = content_dirty && !summary_dirty;

        match git_last_commit(root, &summary_path) {
            Some(last_commit) => {
                let commits_since = git_commit_count(root, Some(&last_commit), &rel_dir_git);
                if commits_since > threshold || has_uncommitted {
                    stale.push(StaleSummary {
                        dir: dir_label,
                        commits_since_update: commits_since,
                        last_summary_commit: last_commit,
                        has_uncommitted_changes: has_uncommitted,
                    });
                }
            }
            None => {
                let total = git_commit_count(root, None, &rel_dir_git);
                if total > 0 || has_uncommitted {
                    missing.push(MissingSummary {
                        dir: dir_label,
                        total_commits: total,
                        has_uncommitted_changes: has_uncommitted,
                    });
                }
            }
        }
    }

    StaleSummaryReport {
        stale,
        missing,
        dirs_checked,
        threshold,
    }
}

impl From<StaleSummaryReport> for DiagnosticsReport {
    fn from(report: StaleSummaryReport) -> Self {
        let threshold = report.threshold;
        let mut issues: Vec<Issue> = report
            .missing
            .into_iter()
            .map(|m| {
                let message = if m.has_uncommitted_changes {
                    format!(
                        "no SUMMARY.md found ({} commits + uncommitted changes touch this directory)",
                        m.total_commits
                    )
                } else {
                    format!(
                        "no SUMMARY.md found ({} commits touch this directory)",
                        m.total_commits
                    )
                };
                Issue {
                    file: if m.dir == "." {
                        "SUMMARY.md".into()
                    } else {
                        format!("{}/SUMMARY.md", m.dir)
                    },
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "missing-summary".into(),
                    message,
                    severity: Severity::Warning,
                    source: "stale-summary".into(),
                    related: vec![],
                    suggestion: Some("add a SUMMARY.md describing this directory's purpose".into()),
                }
            })
            .collect();

        issues.extend(report.stale.into_iter().map(|s| {
            let message = if s.has_uncommitted_changes {
                format!(
                    "{} commits + uncommitted changes since SUMMARY.md was last updated (threshold: {})",
                    s.commits_since_update, threshold
                )
            } else {
                format!(
                    "{} commits since SUMMARY.md was last updated (threshold: {})",
                    s.commits_since_update, threshold
                )
            };
            Issue {
                file: if s.dir == "." {
                    "SUMMARY.md".into()
                } else {
                    format!("{}/SUMMARY.md", s.dir)
                },
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "stale-summary".into(),
                message,
                severity: Severity::Info,
                source: "stale-summary".into(),
                related: vec![],
                suggestion: Some(format!(
                    "update {}/SUMMARY.md to reflect recent changes",
                    s.dir
                )),
            }
        }));

        DiagnosticsReport {
            issues,
            files_checked: report.dirs_checked,
            sources_run: vec!["stale-summary".into()],
        }
    }
}
