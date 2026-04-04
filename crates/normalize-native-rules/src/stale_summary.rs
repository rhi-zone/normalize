use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;

/// Open the git repository at or containing `root` using gix.
fn gix_open(root: &Path) -> Option<gix::Repository> {
    gix::discover(root).ok()
}

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
    let repo = gix_open(root)?;
    let id = repo.head_id().ok()?;
    let s = id.to_hex().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Compute summary freshness for all directories in a single git history pass.
///
/// For each directory label in `dirs`, computes:
/// - `last_doc_commit`: the most recent commit hash that touched any doc file in that dir
/// - `commits_count`: the number of commits touching that dir since `last_doc_commit`
///   (or all commits touching that dir if no doc file has ever been committed)
///
/// This replaces the per-directory `git_last_commit` + `git_commit_count` approach,
/// which required O(dirs × history_length) git tree diffs. A single pass is O(history_length).
///
/// `doc_filenames` is the set of doc file basenames (e.g. `["SUMMARY.md", "CLAUDE.md"]`).
/// `dirs` maps dir_label → (dir_path, is_root) for all directories to track.
fn git_batch_commit_stats(
    root: &Path,
    dirs: &HashMap<String, (String, bool)>,
    doc_filenames: &[&str],
) -> HashMap<String, CacheEntry> {
    let Some(repo) = gix_open(root) else {
        return HashMap::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return HashMap::new();
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return HashMap::new();
    };

    // Per-dir state: (last_doc_commit, commits_since_doc, found_doc)
    // We count commits touching the dir BEFORE we've found the last doc commit.
    // Once we find the doc commit (walking newest-first), we stop counting for that dir.
    struct DirState {
        last_doc_commit: Option<String>,
        commits_since_doc: usize, // commits touching dir before doc commit found
        doc_found: bool,
    }

    let mut states: HashMap<&str, DirState> = dirs
        .keys()
        .map(|label| {
            (
                label.as_str(),
                DirState {
                    last_doc_commit: None,
                    commits_since_doc: 0,
                    doc_found: false,
                },
            )
        })
        .collect();

    // Build lookup: dir_label → (rel_dir_prefix, is_root)
    let dir_info: Vec<(&str, &str, bool)> = dirs
        .iter()
        .map(|(label, (rel_dir, is_root))| (label.as_str(), rel_dir.as_str(), *is_root))
        .collect();

    for info in walk {
        let Ok(info) = info else { continue };
        let Ok(commit) = info.object() else { continue };
        let Ok(tree) = commit.tree() else { continue };
        let parent_tree = info
            .parent_ids()
            .next()
            .and_then(|pid| pid.object().ok())
            .and_then(|obj| obj.into_commit().tree().ok());
        let changes = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Collect changed paths from this commit once.
        let changed_paths: Vec<Vec<u8>> = changes
            .iter()
            .map(|change| {
                use gix::object::tree::diff::ChangeDetached;
                let loc: &[u8] = match &change {
                    ChangeDetached::Addition { location, .. }
                    | ChangeDetached::Deletion { location, .. }
                    | ChangeDetached::Modification { location, .. } => location.as_slice(),
                    ChangeDetached::Rewrite {
                        source_location, ..
                    } => source_location.as_slice(),
                };
                loc.to_vec()
            })
            .collect();

        let commit_sha = info.id.to_hex().to_string();

        for (label, rel_dir, is_root) in &dir_info {
            let state = states.get_mut(*label).unwrap();
            if state.doc_found {
                continue; // already resolved this dir
            }

            // Check if this commit touches the directory.
            let touches_dir = if *is_root {
                !changed_paths.is_empty()
            } else {
                changed_paths.iter().any(|loc| {
                    // loc starts with "rel_dir/" (include the slash to avoid false prefix matches)
                    let prefix = rel_dir.as_bytes();
                    loc.starts_with(prefix)
                        && (loc.len() == prefix.len() || loc.get(prefix.len()) == Some(&b'/'))
                })
            };

            if !touches_dir {
                continue;
            }

            // Check if this commit touches a doc file in this directory.
            let touches_doc = changed_paths.iter().any(|loc| {
                let loc_str = std::str::from_utf8(loc).unwrap_or("");
                doc_filenames.iter().any(|doc| {
                    if *is_root {
                        loc_str == *doc
                    } else {
                        let expected = format!("{}/{}", rel_dir, doc);
                        loc_str == expected
                    }
                })
            });

            if touches_doc {
                // This is the most recent doc commit for this dir.
                state.last_doc_commit = Some(commit_sha.clone());
                state.doc_found = true;
                // commits_since_doc is already the count before this commit — correct.
            } else {
                // Commit touches dir but not a doc file — counts as a "stale" commit.
                state.commits_since_doc += 1;
            }
        }
    }

    // Convert to CacheEntry format.
    states
        .into_iter()
        .map(|(label, state)| {
            (
                label.to_string(),
                CacheEntry {
                    last_summary_commit: state.last_doc_commit,
                    commits_count: state.commits_since_doc,
                },
            )
        })
        .collect()
}

/// Update `existing` cache entries in-place by walking only the commits between
/// `since_sha` (exclusive) and the repository HEAD (inclusive).
///
/// This avoids re-walking all of git history on every pre-commit run after the first commit
/// that follows a cache build. Only the new commits are traversed; existing entries are
/// updated by incrementing `commits_count` for content commits and resetting it for doc commits.
///
/// Walking is newest-first (same order as `git_batch_commit_stats`). The stop condition is
/// reaching `since_sha`. For each directory touched by a new commit:
/// - If a doc file is touched and no doc commit has been recorded in this incremental walk yet:
///   set `last_summary_commit` to this commit, reset `commits_count` to 0. This is the newest
///   doc commit for this directory in the new range.
/// - If a doc file is touched but a newer doc commit was already recorded in this walk: ignore
///   (this is an older doc commit, already superseded).
/// - If only content is touched and no doc commit has been recorded in this walk yet:
///   increment `commits_count` (this content commit is newer than the last doc commit).
/// - If only content is touched and a doc commit was already recorded in this walk: ignore
///   (this commit is older than the new doc commit and its count is already captured in the
///   existing `commits_count` from the previous full walk).
///
/// **Key invariant**: after an incremental update, `CacheEntry.commits_count` is the number
/// of content commits since `last_summary_commit` — the same semantics as a full walk.
fn git_incremental_commit_stats(
    root: &Path,
    since_sha: &str,
    existing: &mut HashMap<String, CacheEntry>,
    dirs: &HashMap<String, (String, bool)>,
    doc_filenames: &[&str],
) {
    let Some(repo) = gix_open(root) else {
        return;
    };
    let Ok(head_id) = repo.head_id() else {
        return;
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return;
    };

    // Per-dir incremental state: did we already see a new doc commit for this dir?
    // `new_doc_found` is set the first time we encounter a doc commit in the new range.
    struct IncrState {
        new_doc_found: bool,
    }
    let mut inc_states: HashMap<&str, IncrState> = dirs
        .keys()
        .map(|label| {
            (
                label.as_str(),
                IncrState {
                    new_doc_found: false,
                },
            )
        })
        .collect();

    // Build lookup: dir_label → (rel_dir_prefix, is_root)
    let dir_info: Vec<(&str, &str, bool)> = dirs
        .iter()
        .map(|(label, (rel_dir, is_root))| (label.as_str(), rel_dir.as_str(), *is_root))
        .collect();

    for info in walk {
        let Ok(info) = info else { continue };

        let commit_sha = info.id.to_hex().to_string();
        // Stop when we reach the previously cached HEAD (exclusive lower bound).
        if commit_sha == since_sha {
            break;
        }

        let Ok(commit) = info.object() else { continue };
        let Ok(tree) = commit.tree() else { continue };
        let parent_tree = info
            .parent_ids()
            .next()
            .and_then(|pid| pid.object().ok())
            .and_then(|obj| obj.into_commit().tree().ok());
        let changes = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let changed_paths: Vec<Vec<u8>> = changes
            .iter()
            .map(|change| {
                use gix::object::tree::diff::ChangeDetached;
                let loc: &[u8] = match &change {
                    ChangeDetached::Addition { location, .. }
                    | ChangeDetached::Deletion { location, .. }
                    | ChangeDetached::Modification { location, .. } => location.as_slice(),
                    ChangeDetached::Rewrite {
                        source_location, ..
                    } => source_location.as_slice(),
                };
                loc.to_vec()
            })
            .collect();

        for (label, rel_dir, is_root) in &dir_info {
            let inc = inc_states.get_mut(*label).unwrap();

            // Check if this commit touches the directory.
            let touches_dir = if *is_root {
                !changed_paths.is_empty()
            } else {
                changed_paths.iter().any(|loc| {
                    let prefix = rel_dir.as_bytes();
                    loc.starts_with(prefix)
                        && (loc.len() == prefix.len() || loc.get(prefix.len()) == Some(&b'/'))
                })
            };

            if !touches_dir {
                continue;
            }

            // Check if this commit touches a doc file in this directory.
            let touches_doc = changed_paths.iter().any(|loc| {
                let loc_str = std::str::from_utf8(loc).unwrap_or("");
                doc_filenames.iter().any(|doc| {
                    if *is_root {
                        loc_str == *doc
                    } else {
                        let expected = format!("{}/{}", rel_dir, doc);
                        loc_str == expected
                    }
                })
            });

            if touches_doc && !inc.new_doc_found {
                // Newest doc commit in the new range: reset the entry.
                inc.new_doc_found = true;
                let entry = existing.entry(label.to_string()).or_insert(CacheEntry {
                    last_summary_commit: None,
                    commits_count: 0,
                });
                entry.last_summary_commit = Some(commit_sha.clone());
                entry.commits_count = 0;
            } else if !touches_doc && !inc.new_doc_found {
                // Content commit newer than any new doc commit: increment counter.
                let entry = existing.entry(label.to_string()).or_insert(CacheEntry {
                    last_summary_commit: None,
                    commits_count: 0,
                });
                entry.commits_count += 1;
            }
            // touches_doc && inc.new_doc_found → older doc commit, ignore.
            // !touches_doc && inc.new_doc_found → older content commit, ignore.
        }
    }
}

/// All paths with uncommitted changes (staged or unstaged), collected once for the whole repo.
///
/// Built once before the directory loop in both report builders; all per-directory checks
/// then become pure in-memory prefix/membership tests — no further git I/O per directory.
struct UncommittedChanges {
    /// Paths (relative to repo root) with staged changes (index differs from HEAD).
    staged: HashSet<String>,
    /// Paths (relative to repo root) with unstaged changes (worktree differs from index).
    unstaged: HashSet<String>,
}

impl UncommittedChanges {
    /// Build once for the whole repo.  Opens the repository, reads the index and HEAD tree
    /// for staged changes, then runs a single worktree status walk for unstaged changes.
    fn load(root: &Path) -> Self {
        let Some(repo) = gix_open(root) else {
            return Self {
                staged: HashSet::new(),
                unstaged: HashSet::new(),
            };
        };

        // Staged: collect all index entries that differ from HEAD.
        let staged = (|| -> Option<HashSet<String>> {
            use gix::bstr::ByteSlice;
            let head_id = repo.head_id().ok()?;
            let head_commit = head_id.object().ok()?.into_commit();
            let head_tree = head_commit.tree().ok()?;
            let index = repo.index_or_empty().ok()?;
            let mut set = HashSet::new();
            for entry in index.entries() {
                let rela = entry.path(&index);
                let rela_str = rela.to_str_lossy();
                let head_blob_id = head_tree
                    .lookup_entry_by_path(rela_str.as_ref())
                    .ok()
                    .flatten()
                    .map(|e| e.id().detach());
                // Present in index but not HEAD (new file), or different blob id = staged change.
                if head_blob_id.as_ref() != Some(&entry.id) {
                    set.insert(rela_str.into_owned());
                }
            }
            Some(set)
        })()
        .unwrap_or_default();

        // Unstaged: single status walk over the whole worktree with no path patterns.
        let unstaged = (|| -> Option<HashSet<String>> {
            use gix::bstr::ByteSlice;
            let platform = repo
                .status(gix::progress::Discard)
                .ok()?
                .index_worktree_options_mut(|opts| {
                    opts.dirwalk_options = None;
                });
            let iter = platform
                .into_index_worktree_iter(Vec::<gix::bstr::BString>::new())
                .ok()?;
            let mut set = HashSet::new();
            for item in iter.flatten() {
                let rela = item.rela_path().to_str_lossy();
                set.insert(rela.into_owned());
            }
            Some(set)
        })()
        .unwrap_or_default();

        Self { staged, unstaged }
    }

    /// Returns true if any changed file under `rel_dir` (excluding `doc_paths`) exists.
    ///
    /// Used to detect content changes that should trigger a doc-freshness warning.
    fn has_content_changes(&self, rel_dir: &str, doc_paths: &[String]) -> bool {
        let is_root = rel_dir == ".";
        let check = |path: &str| -> bool {
            if !is_root && !path.starts_with(rel_dir) {
                return false;
            }
            !doc_paths.iter().any(|dp| dp.as_str() == path)
        };
        self.staged.iter().any(|p| check(p)) || self.unstaged.iter().any(|p| check(p))
    }

    /// Returns true if the given doc file path itself has uncommitted changes.
    ///
    /// Used to skip stale/missing reporting when the doc is already being updated.
    fn summary_has_changes(&self, summary_path: &str) -> bool {
        self.staged.contains(summary_path) || self.unstaged.contains(summary_path)
    }
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
fn walk_dirs(
    root: &Path,
    walk_config: &normalize_rules_config::WalkConfig,
) -> Vec<(std::path::PathBuf, String)> {
    crate::walk::gitignore_walk(root, walk_config)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_dir()))
        .filter(|e| {
            !e.path()
                .components()
                .any(|c| c.as_os_str() == OsStr::new(".git"))
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
    walk_config: &normalize_rules_config::WalkConfig,
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
    let mut cache = load_cache(root);

    // If the cache exists but HEAD has moved, walk only the new commits and update in-place.
    if let (Some(c), Some(current_head)) = (&mut cache, &head)
        && c.head != *current_head
    {
        // Build the full dirs map (all directories, not just uncached ones) so the
        // incremental walk can update any entry that was touched by the new commits.
        let all_dirs: HashMap<String, (String, bool)> = {
            let dirs_snapshot = walk_dirs(root, walk_config);
            dirs_snapshot
                .iter()
                .map(|(dir_path, dir_label)| {
                    let rel = dir_path
                        .strip_prefix(root)
                        .unwrap_or(dir_path)
                        .to_string_lossy();
                    let rel_dir = if rel.is_empty() {
                        ".".to_string()
                    } else {
                        rel.to_string()
                    };
                    let is_root = rel_dir == ".";
                    (dir_label.clone(), (rel_dir, is_root))
                })
                .collect()
        };
        git_incremental_commit_stats(root, &c.head, &mut c.dirs, &all_dirs, &filenames);
        c.head = current_head.clone();
    }

    let mut updated_dirs: HashMap<String, CacheEntry> = HashMap::new();

    let dirs = walk_dirs(root, walk_config);

    // Identify directories not covered by the cache — compute their stats in a single
    // git history pass rather than one per directory.
    let uncached_dirs: HashMap<String, (String, bool)> = dirs
        .iter()
        .filter(|(_, dir_label)| {
            dir_matches_paths(dir_label, paths)
                && cache
                    .as_ref()
                    .is_none_or(|c| !c.dirs.contains_key(dir_label))
        })
        .map(|(dir_path, dir_label)| {
            let rel = dir_path
                .strip_prefix(root)
                .unwrap_or(dir_path)
                .to_string_lossy();
            let rel_dir = if rel.is_empty() {
                ".".to_string()
            } else {
                rel.to_string()
            };
            let is_root = rel_dir == ".";
            (dir_label.clone(), (rel_dir, is_root))
        })
        .collect();

    let batch_results = if uncached_dirs.is_empty() {
        HashMap::new()
    } else {
        git_batch_commit_stats(root, &uncached_dirs, &filenames)
    };

    // Collect all uncommitted changes once before the loop to avoid per-directory git I/O.
    let uncommitted = UncommittedChanges::load(root);

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

        // Always re-check for uncommitted content changes (in-memory after the batched load).
        let content_dirty = uncommitted.has_content_changes(&rel_dir_git, &candidate_paths);

        // If ANY candidate doc file is staged (about to be committed), skip the check.
        let any_doc_dirty = candidate_paths
            .iter()
            .any(|p| uncommitted.summary_has_changes(p));
        if any_doc_dirty {
            continue;
        }

        let (last_summary_commit, commits_count) =
            if let Some(entry) = cache.as_ref().and_then(|c| c.dirs.get(dir_label)) {
                (entry.last_summary_commit.clone(), entry.commits_count)
            } else if let Some(entry) = batch_results.get(dir_label) {
                (entry.last_summary_commit.clone(), entry.commits_count)
            } else {
                (None, 0)
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
    walk_config: &normalize_rules_config::WalkConfig,
) -> StaleSummaryReport {
    let filenames: Vec<&str> = if filenames.is_empty() {
        DEFAULT_FILENAMES.to_vec()
    } else {
        filenames.iter().map(String::as_str).collect()
    };
    let mut stale = Vec::new();
    let mut dirs_checked = 0;

    // Load incremental cache: if HEAD has moved since the last run, walk only the new commits
    // and update the cached entries in-place rather than re-walking all of history.
    let head = git_head(root);
    let mut cache = load_cache(root);

    if let (Some(c), Some(current_head)) = (&mut cache, &head)
        && c.head != *current_head
    {
        let all_dirs: HashMap<String, (String, bool)> = {
            let dirs_snapshot = walk_dirs(root, walk_config);
            dirs_snapshot
                .iter()
                .map(|(dir_path, dir_label)| {
                    let rel = dir_path
                        .strip_prefix(root)
                        .unwrap_or(dir_path)
                        .to_string_lossy();
                    let rel_dir = if rel.is_empty() {
                        ".".to_string()
                    } else {
                        rel.to_string()
                    };
                    let is_root = rel_dir == ".";
                    (dir_label.clone(), (rel_dir, is_root))
                })
                .collect()
        };
        git_incremental_commit_stats(root, &c.head, &mut c.dirs, &all_dirs, &filenames);
        c.head = current_head.clone();
    }

    let mut updated_dirs: HashMap<String, CacheEntry> = HashMap::new();

    let dirs = walk_dirs(root, walk_config);

    // Identify directories not covered by the cache — compute their stats in a single
    // git history pass rather than one per directory.
    let uncached_dirs: HashMap<String, (String, bool)> = dirs
        .iter()
        .filter(|(_, dir_label)| {
            dir_matches_paths(dir_label, paths)
                && cache
                    .as_ref()
                    .is_none_or(|c| !c.dirs.contains_key(dir_label))
        })
        .map(|(dir_path, dir_label)| {
            let rel = dir_path
                .strip_prefix(root)
                .unwrap_or(dir_path)
                .to_string_lossy();
            let rel_dir = if rel.is_empty() {
                ".".to_string()
            } else {
                rel.to_string()
            };
            let is_root = rel_dir == ".";
            (dir_label.clone(), (rel_dir, is_root))
        })
        .collect();

    let batch_results = if uncached_dirs.is_empty() {
        HashMap::new()
    } else {
        git_batch_commit_stats(root, &uncached_dirs, &filenames)
    };

    // Collect all uncommitted changes once before the loop to avoid per-directory git I/O.
    let uncommitted = UncommittedChanges::load(root);

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

        // Always re-check for uncommitted content changes (in-memory after the batched load).
        // "content_dirty" excludes all candidate doc files from the signal.
        let content_dirty = uncommitted.has_content_changes(&rel_dir_git, &candidate_paths);

        // If ANY candidate doc file is staged (about to be committed), skip the
        // staleness check: the pending commit will fix it.
        let any_doc_dirty = candidate_paths
            .iter()
            .any(|p| uncommitted.summary_has_changes(p));
        if any_doc_dirty {
            continue;
        }

        // For OR semantics: find the candidate that has the most recent commit
        // (smallest commits_since_update). If none have ever been committed,
        // the directory is treated as missing — skip it here (handled by missing-summary).
        //
        // Cache key: dir_label — we store the best result across all candidates.
        let (last_summary_commit, commits_count) =
            if let Some(entry) = cache.as_ref().and_then(|c| c.dirs.get(dir_label)) {
                (entry.last_summary_commit.clone(), entry.commits_count)
            } else if let Some(entry) = batch_results.get(dir_label) {
                (entry.last_summary_commit.clone(), entry.commits_count)
            } else {
                (None, 0)
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
            daemon_cached: false,
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
            daemon_cached: false,
        }
    }
}
