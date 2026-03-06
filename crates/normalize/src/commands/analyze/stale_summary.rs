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
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct MissingSummary {
    dir: String,
    total_commits: usize,
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
                    lines.push(format!(
                        "  {} ({} commits with no SUMMARY.md)",
                        m.dir, m.total_commits
                    ));
                }
                lines.push(String::new());
            }
            if !self.stale.is_empty() {
                lines.push(format!("Stale SUMMARY.md ({}):", self.stale.len()));
                for s in &self.stale {
                    lines.push(format!(
                        "  {} ({} commits since last update)",
                        s.dir, s.commits_since_update
                    ));
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
        // Use "." for the root directory in git commands
        let rel_dir_git = if rel_dir.is_empty() {
            ".".to_string()
        } else {
            rel_dir.to_string()
        };

        // Only check dirs that have at least one tracked file directly in them
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

        match git_last_commit(root, &summary_path) {
            Some(last_commit) => {
                let commits_since = git_commit_count(root, Some(&last_commit), &rel_dir_git);
                if commits_since > threshold {
                    stale.push(StaleSummary {
                        dir: if rel_dir.is_empty() {
                            ".".to_string()
                        } else {
                            rel_dir.to_string()
                        },
                        commits_since_update: commits_since,
                        last_summary_commit: last_commit,
                    });
                }
            }
            None => {
                // SUMMARY.md never committed — count all commits to this dir
                let total = git_commit_count(root, None, &rel_dir_git);
                if total > 0 {
                    missing.push(MissingSummary {
                        dir: if rel_dir.is_empty() {
                            ".".to_string()
                        } else {
                            rel_dir.to_string()
                        },
                        total_commits: total,
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
            .map(|m| Issue {
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
                message: format!(
                    "no SUMMARY.md found ({} commits touch this directory)",
                    m.total_commits
                ),
                severity: Severity::Warning,
                source: "stale-summary".into(),
                related: vec![],
                suggestion: Some("add a SUMMARY.md describing this directory's purpose".into()),
            })
            .collect();

        issues.extend(report.stale.into_iter().map(|s| Issue {
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
            message: format!(
                "{} commits since SUMMARY.md was last updated (threshold: {})",
                s.commits_since_update, threshold
            ),
            severity: Severity::Info,
            source: "stale-summary".into(),
            related: vec![],
            suggestion: Some(format!(
                "update {}/SUMMARY.md to reflect recent changes",
                s.dir
            )),
        }));

        DiagnosticsReport {
            issues,
            files_checked: report.dirs_checked,
            sources_run: vec!["stale-summary".into()],
        }
    }
}
