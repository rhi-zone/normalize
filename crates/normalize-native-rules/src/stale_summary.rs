use crate::walk::is_excluded_dir;
use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StaleSummary {
    dir: String,
    commits_since_update: usize,
    last_summary_commit: String,
    /// True if the directory has uncommitted changes not reflected in the doc file.
    has_uncommitted_changes: bool,
    /// The doc filename that was found (e.g. "SUMMARY.md" or "CLAUDE.md").
    filename: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct MissingSummary {
    dir: String,
    total_commits: usize,
    /// True if the directory has uncommitted changes with no doc file at all.
    has_uncommitted_changes: bool,
    /// The candidate doc filenames that were checked (none were found).
    filenames: Vec<String>,
}

/// Report produced by the `missing-summary` native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MissingSummaryReport {
    missing: Vec<MissingSummary>,
    dirs_checked: usize,
    threshold: usize,
}

impl OutputFormatter for MissingSummaryReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Doc File Presence Check".to_string());
        lines.push(String::new());
        lines.push(format!("Directories checked: {}", self.dirs_checked));
        lines.push(format!("Commit threshold: {}", self.threshold));
        lines.push(String::new());

        if self.missing.is_empty() {
            lines.push("All directories have a doc file.".to_string());
        } else {
            lines.push(format!("Missing doc file ({}):", self.missing.len()));
            for m in &self.missing {
                let candidates = m.filenames.join(" or ");
                let suffix = if m.has_uncommitted_changes {
                    format!(
                        "{} commits + uncommitted changes, no {}",
                        m.total_commits, candidates
                    )
                } else {
                    format!("{} commits with no {}", m.total_commits, candidates)
                };
                lines.push(format!("  {} ({})", m.dir, suffix));
            }
        }

        lines.join("\n")
    }
}

/// Report produced by the `stale-summary` native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StaleSummaryReport {
    stale: Vec<StaleSummary>,
    dirs_checked: usize,
    threshold: usize,
}

impl OutputFormatter for StaleSummaryReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Doc File Freshness Check".to_string());
        lines.push(String::new());
        lines.push(format!("Directories checked: {}", self.dirs_checked));
        lines.push(format!("Staleness threshold: {} commits", self.threshold));
        lines.push(String::new());

        if self.stale.is_empty() {
            lines.push("All doc files are up to date.".to_string());
        } else {
            lines.push(format!("Stale doc file ({}):", self.stale.len()));
            for s in &self.stale {
                let suffix = if s.has_uncommitted_changes {
                    format!(
                        "{} commits + uncommitted changes since {} last updated",
                        s.commits_since_update, s.filename
                    )
                } else {
                    format!(
                        "{} commits since {} last updated",
                        s.commits_since_update, s.filename
                    )
                };
                lines.push(format!("  {} ({})", s.dir, suffix));
            }
        }

        lines.join("\n")
    }
}

// --- Incremental cache ---

/// One cached entry per directory, keyed by relative dir path.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    /// Last commit hash touching SUMMARY.md, or None if no SUMMARY.md has ever been committed.
    last_summary_commit: Option<String>,
    /// Commits touching this dir since `last_summary_commit` (exclusive), or total commits if
    /// `last_summary_commit` is None.
    commits_count: usize,
}

/// Cache file stored at `.normalize/cache/summary-freshness.json`.
#[derive(Debug, Serialize, Deserialize)]
struct SummaryCache {
    /// HEAD commit hash when this cache was written.
    head: String,
    dirs: HashMap<String, CacheEntry>,
}

fn cache_path(root: &Path) -> std::path::PathBuf {
    root.join(".normalize/cache/summary-freshness.json")
}

fn load_cache(root: &Path) -> Option<SummaryCache> {
    let path = cache_path(root);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return None, // missing cache file is normal
    };
    match serde_json::from_str(&content) {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::debug!(
                "normalize-native-rules: corrupt summary cache at {:?}: {}",
                path,
                e
            );
            None
        }
    }
}

fn save_cache(root: &Path, cache: &SummaryCache) {
    let dir = root.join(".normalize/cache");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(cache_path(root), json);
    }
}

fn git_head(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", root.to_str().unwrap_or("."), "rev-parse", "HEAD"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
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
/// NOT limited to doc files themselves (i.e., real content changes needing documentation).
///
/// `doc_paths` lists the relative paths of doc files to exclude from the signal
/// (e.g. `["SUMMARY.md", "src/SUMMARY.md"]`).
fn git_has_uncommitted_content_changes(root: &Path, rel_dir: &str, doc_paths: &[String]) -> bool {
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
            // Exclude all candidate doc files from the "content changed" signal
            !doc_paths.iter().any(|dp| dp.as_str() == path)
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

/// Default filenames checked by `stale-summary` and `missing-summary` when none are configured.
pub const DEFAULT_FILENAMES: &[&str] = &["SUMMARY.md"];

/// Returns true if `dir_label` matches any of the `paths` glob patterns.
///
/// A leading `./` in `dir_label` is stripped before matching. If `paths` is empty,
/// returns `true` (the rule applies everywhere).
fn dir_matches_paths(dir_label: &str, paths: &[String]) -> bool {
    if paths.is_empty() {
        return true;
    }
    // Normalize: strip leading "./" for matching
    let label = dir_label.strip_prefix("./").unwrap_or(dir_label);
    // The root dir "." matches a bare "." pattern only; for non-root dirs we match
    // the label against each glob pattern.
    paths.iter().any(|pat| {
        glob::Pattern::new(pat)
            .map(|p| p.matches(label))
            .unwrap_or(false)
    })
}

/// Shared directory walker used by both report builders.
///
/// Yields `(dir_path, rel_dir_str, rel_dir_git, dir_label)` tuples for every
/// non-empty directory in the repository tree (after excluding VCS/build dirs).
fn walk_dirs(root: &Path) -> Vec<(std::path::PathBuf, String)> {
    crate::walk::gitignore_walk(root)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_dir()))
        .filter(|e| {
            !e.path()
                .components()
                .any(|c| is_excluded_dir(c.as_os_str().to_string_lossy().as_ref()))
        })
        .filter_map(|e| {
            let dir_path = e.path().to_path_buf();
            let has_files = std::fs::read_dir(&dir_path)
                .map(|mut rd| {
                    rd.any(|e| {
                        e.map(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if !has_files {
                return None;
            }
            let rel = dir_path
                .strip_prefix(root)
                .unwrap_or(&dir_path)
                .to_string_lossy();
            let label = if rel.is_empty() {
                ".".to_string()
            } else {
                rel.to_string()
            };
            Some((dir_path, label))
        })
        .collect()
}

/// Build a [`MissingSummaryReport`] by walking the repository under `root` and checking
/// each directory for a doc file that is present (committed at least once).
///
/// `filenames` lists the candidate doc filenames (e.g. `["SUMMARY.md", "CLAUDE.md"]`).
/// A directory is compliant when it has **any** of those files (OR semantics).
/// Pass an empty slice to fall back to [`DEFAULT_FILENAMES`].
///
/// `paths` is a list of glob patterns; only directories matching one of the patterns are
/// checked. An empty `paths` slice means the rule applies to every directory (default behavior).
///
/// Directories that have never had a doc file committed are reported as missing when
/// the total commit count (plus any uncommitted content changes) exceeds `threshold`.
pub fn build_missing_summary_report(
    root: &Path,
    threshold: usize,
    filenames: &[String],
    paths: &[String],
) -> MissingSummaryReport {
    let filenames: Vec<&str> = if filenames.is_empty() {
        DEFAULT_FILENAMES.to_vec()
    } else {
        filenames.iter().map(String::as_str).collect()
    };
    let mut missing = Vec::new();
    let mut dirs_checked = 0;

    // Load incremental cache (shared with stale-summary to avoid redundant git calls).
    let head = git_head(root);
    let mut cache = head
        .as_deref()
        .and_then(|h| load_cache(root).filter(|c| c.head == h));
    let mut updated_dirs: HashMap<String, CacheEntry> = HashMap::new();

    let dirs = walk_dirs(root);

    for (dir_path, dir_label) in &dirs {
        // Apply paths filter: skip directories that don't match any configured glob.
        if !dir_matches_paths(dir_label, paths) {
            continue;
        }

        let rel_dir = dir_path
            .strip_prefix(root)
            .unwrap_or(dir_path)
            .to_string_lossy();
        let rel_dir_git = if rel_dir.is_empty() {
            ".".to_string()
        } else {
            rel_dir.to_string()
        };

        // Build the relative paths for each candidate filename.
        let candidate_paths: Vec<String> = filenames
            .iter()
            .map(|f| {
                if rel_dir.is_empty() {
                    f.to_string()
                } else {
                    format!("{}/{}", rel_dir, f)
                }
            })
            .collect();

        // git status is cheap — always re-check for uncommitted changes.
        let content_dirty =
            git_has_uncommitted_content_changes(root, &rel_dir_git, &candidate_paths);

        // If ANY candidate doc file is staged (about to be committed), skip the check.
        let any_doc_dirty = candidate_paths
            .iter()
            .any(|p| git_summary_has_uncommitted_changes(root, p));
        if any_doc_dirty {
            continue;
        }

        let cached = cache.as_ref().and_then(|c| c.dirs.get(dir_label));
        let (last_summary_commit, commits_count) = if let Some(entry) = cached {
            (entry.last_summary_commit.clone(), entry.commits_count)
        } else {
            let mut best: Option<(String, usize)> = None;
            for doc_path in &candidate_paths {
                if let Some(last) = git_last_commit(root, doc_path) {
                    let count = git_commit_count(root, Some(&last), &rel_dir_git);
                    best = Some(match best {
                        None => (last, count),
                        Some((prev_hash, prev_count)) => {
                            if count <= prev_count {
                                (last, count)
                            } else {
                                (prev_hash, prev_count)
                            }
                        }
                    });
                }
            }
            match best {
                Some((hash, count)) => (Some(hash), count),
                None => {
                    let count = git_commit_count(root, None, &rel_dir_git);
                    (None, count)
                }
            }
        };

        updated_dirs.insert(
            dir_label.clone(),
            CacheEntry {
                last_summary_commit: last_summary_commit.clone(),
                commits_count,
            },
        );

        let effective_count = commits_count + usize::from(content_dirty);

        // missing-summary only fires when there is NO committed doc file.
        if last_summary_commit.is_none() && effective_count > threshold {
            dirs_checked += 1;
            missing.push(MissingSummary {
                dir: dir_label.clone(),
                total_commits: commits_count,
                has_uncommitted_changes: content_dirty,
                filenames: filenames.iter().map(|s| s.to_string()).collect(),
            });
        } else {
            dirs_checked += 1;
        }
    }

    // Persist updated cache.
    if let Some(head_hash) = head {
        let merged_dirs = if let Some(ref mut old) = cache {
            old.dirs.extend(updated_dirs);
            std::mem::take(&mut old.dirs)
        } else {
            updated_dirs
        };
        save_cache(
            root,
            &SummaryCache {
                head: head_hash,
                dirs: merged_dirs,
            },
        );
    }

    MissingSummaryReport {
        missing,
        dirs_checked,
        threshold,
    }
}

/// Build a [`StaleSummaryReport`] by walking the repository under `root` and checking
/// each directory for a doc file that is up-to-date.
///
/// `filenames` lists the candidate doc filenames (e.g. `["SUMMARY.md", "CLAUDE.md"]`).
/// A directory is compliant when it has **any** of those files and none of the present
/// ones are stale (OR semantics). Pass an empty slice to fall back to [`DEFAULT_FILENAMES`].
///
/// `paths` is a list of glob patterns; only directories matching one of the patterns are
/// checked. An empty `paths` slice means the rule applies to every directory (default behavior).
///
/// A doc file is considered stale when the number of commits since its last update (plus any
/// uncommitted content changes in the directory) exceeds `threshold`. Directories without any
/// matching doc file are NOT reported here — use `build_missing_summary_report` for that.
pub fn build_stale_summary_report(
    root: &Path,
    threshold: usize,
    filenames: &[String],
    paths: &[String],
) -> StaleSummaryReport {
    let filenames: Vec<&str> = if filenames.is_empty() {
        DEFAULT_FILENAMES.to_vec()
    } else {
        filenames.iter().map(String::as_str).collect()
    };
    let mut stale = Vec::new();
    let mut dirs_checked = 0;

    // Load incremental cache: keyed by HEAD commit, avoids per-dir `git log` on repeat runs.
    let head = git_head(root);
    let mut cache = head
        .as_deref()
        .and_then(|h| load_cache(root).filter(|c| c.head == h));
    let mut updated_dirs: HashMap<String, CacheEntry> = HashMap::new();

    let dirs = walk_dirs(root);

    for (dir_path, dir_label) in &dirs {
        // Apply paths filter: skip directories that don't match any configured glob.
        if !dir_matches_paths(dir_label, paths) {
            continue;
        }

        let rel_dir = dir_path
            .strip_prefix(root)
            .unwrap_or(dir_path)
            .to_string_lossy();
        let rel_dir_git = if rel_dir.is_empty() {
            ".".to_string()
        } else {
            rel_dir.to_string()
        };

        dirs_checked += 1;

        // Build the relative paths for each candidate filename.
        let candidate_paths: Vec<String> = filenames
            .iter()
            .map(|f| {
                if rel_dir.is_empty() {
                    f.to_string()
                } else {
                    format!("{}/{}", rel_dir, f)
                }
            })
            .collect();

        // git status is cheap — always re-check for uncommitted changes.
        // "content_dirty" excludes all candidate doc files from the signal.
        let content_dirty =
            git_has_uncommitted_content_changes(root, &rel_dir_git, &candidate_paths);

        // If ANY candidate doc file is staged (about to be committed), skip the
        // staleness check: the pending commit will fix it.
        let any_doc_dirty = candidate_paths
            .iter()
            .any(|p| git_summary_has_uncommitted_changes(root, p));
        if any_doc_dirty {
            continue;
        }

        // For OR semantics: find the candidate that has the most recent commit
        // (smallest commits_since_update). If none have ever been committed,
        // the directory is treated as missing — skip it here (handled by missing-summary).
        //
        // Cache key: dir_label — we store the best result across all candidates.
        let cached = cache.as_ref().and_then(|c| c.dirs.get(dir_label));
        let (last_summary_commit, commits_count) = if let Some(entry) = cached {
            (entry.last_summary_commit.clone(), entry.commits_count)
        } else {
            // Try each candidate filename; pick the one with the fewest commits
            // since its last update (i.e. the freshest doc file).
            let mut best: Option<(String, usize)> = None; // (commit_hash, count)
            for doc_path in &candidate_paths {
                if let Some(last) = git_last_commit(root, doc_path) {
                    let count = git_commit_count(root, Some(&last), &rel_dir_git);
                    best = Some(match best {
                        None => (last, count),
                        Some((prev_hash, prev_count)) => {
                            if count <= prev_count {
                                (last, count)
                            } else {
                                (prev_hash, prev_count)
                            }
                        }
                    });
                }
            }
            match best {
                Some((hash, count)) => (Some(hash), count),
                None => {
                    // No doc file has ever been committed — not our concern (missing-summary handles it).
                    let count = git_commit_count(root, None, &rel_dir_git);
                    (None, count)
                }
            }
        };

        // Store result for cache write.
        updated_dirs.insert(
            dir_label.clone(),
            CacheEntry {
                last_summary_commit: last_summary_commit.clone(),
                commits_count,
            },
        );

        // Effective change count: committed changes + 1 if there are uncommitted content changes.
        let effective_count = commits_count + usize::from(content_dirty);

        // Display name: first candidate filename (representative for messages).
        let primary_filename = filenames.first().copied().unwrap_or("SUMMARY.md");

        // stale-summary only fires when a doc file EXISTS but is stale.
        if let Some(last_commit) = last_summary_commit
            && effective_count > threshold
        {
            stale.push(StaleSummary {
                dir: dir_label.clone(),
                commits_since_update: commits_count,
                last_summary_commit: last_commit,
                has_uncommitted_changes: content_dirty,
                filename: primary_filename.to_string(),
            });
        }
        // If last_summary_commit is None, the directory is missing a doc file entirely.
        // That is handled by missing-summary, not stale-summary.
    }

    // Persist updated cache (merge with existing to preserve entries not visited this run).
    if let Some(head_hash) = head {
        let merged_dirs = if let Some(ref mut old) = cache {
            old.dirs.extend(updated_dirs);
            std::mem::take(&mut old.dirs)
        } else {
            updated_dirs
        };
        save_cache(
            root,
            &SummaryCache {
                head: head_hash,
                dirs: merged_dirs,
            },
        );
    }

    StaleSummaryReport {
        stale,
        dirs_checked,
        threshold,
    }
}

impl From<MissingSummaryReport> for DiagnosticsReport {
    fn from(report: MissingSummaryReport) -> Self {
        let issues: Vec<Issue> = report
            .missing
            .into_iter()
            .map(|m| {
                let candidates = m.filenames.join(" or ");
                let primary = m
                    .filenames
                    .first()
                    .map(String::as_str)
                    .unwrap_or("SUMMARY.md");
                let message = if m.has_uncommitted_changes {
                    format!(
                        "no {} found ({} commits + uncommitted changes touch this directory)",
                        candidates, m.total_commits
                    )
                } else {
                    format!(
                        "no {} found ({} commits touch this directory)",
                        candidates, m.total_commits
                    )
                };
                Issue {
                    file: format!("{}/{}", m.dir, primary),
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "missing-summary".into(),
                    message,
                    severity: Severity::Error,
                    source: "missing-summary".into(),
                    related: vec![],
                    suggestion: Some(format!(
                        "add a {} describing this directory's purpose",
                        candidates
                    )),
                }
            })
            .collect();

        DiagnosticsReport {
            issues,
            files_checked: report.dirs_checked,
            sources_run: vec!["missing-summary".into()],
            tool_errors: vec![],
        }
    }
}

impl From<StaleSummaryReport> for DiagnosticsReport {
    fn from(report: StaleSummaryReport) -> Self {
        let threshold = report.threshold;

        let issues: Vec<Issue> = report
            .stale
            .into_iter()
            .map(|s| {
                let message = if s.has_uncommitted_changes {
                    format!(
                        "{} commits + uncommitted changes since {} was last updated (threshold: {})",
                        s.commits_since_update, s.filename, threshold
                    )
                } else {
                    format!(
                        "{} commits since {} was last updated (threshold: {})",
                        s.commits_since_update, s.filename, threshold
                    )
                };
                Issue {
                    file: format!("{}/{}", s.dir, s.filename),
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "stale-summary".into(),
                    message,
                    severity: Severity::Error,
                    source: "stale-summary".into(),
                    related: vec![],
                    suggestion: Some(format!(
                        "{}/{} should describe the directory's current purpose, key files, and how they fit together",
                        s.dir, s.filename
                    )),
                }
            })
            .collect();

        DiagnosticsReport {
            issues,
            files_checked: report.dirs_checked,
            sources_run: vec!["stale-summary".into()],
            tool_errors: vec![],
        }
    }
}
