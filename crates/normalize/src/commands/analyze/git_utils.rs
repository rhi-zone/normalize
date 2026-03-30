//! Git utility functions using `gix` (gitoxide) — no PATH dependency.
//!
//! This module provides read-only git operations backed by the pure-Rust `gix` library.
//! Write operations (worktree add/remove) remain as shell-outs in `run_in_worktree`
//! because `gix` does not support those yet.
//!
//! All public functions return `Option` or `Result` and degrade gracefully if the
//! repository cannot be opened (e.g. no `.git` directory, bare repo, or gix error).

use std::collections::HashMap;
use std::path::Path;

// ── Repository open ──────────────────────────────────────────────────────────

/// Open the git repository at or containing `path`.
///
/// Uses `gix::discover` so it walks parent directories just like `git` would.
/// Returns `None` if no repository is found (graceful degradation).
pub fn open_repo(path: &Path) -> Option<gix::Repository> {
    gix::discover(path).ok()
}

// ── HEAD ─────────────────────────────────────────────────────────────────────

/// Return the full SHA-1 hex string of the current HEAD commit, or `None`.
///
/// Equivalent to `git rev-parse HEAD`.
pub fn git_head(root: &Path) -> Option<String> {
    let repo = open_repo(root)?;
    let id = repo.head_id().ok()?;
    Some(id.to_hex().to_string())
}

/// Return the short branch name of HEAD (e.g. `"main"`), or `None` if detached.
///
/// Equivalent to `git rev-parse --abbrev-ref HEAD`.
pub fn git_head_branch(root: &Path) -> Option<String> {
    let repo = open_repo(root)?;
    let head = repo.head().ok()?;
    let name = head.referent_name()?;
    // Strip the "refs/heads/" prefix to get the short branch name.
    let short = name
        .as_bstr()
        .strip_prefix(b"refs/heads/")
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_else(|| String::from_utf8_lossy(name.as_bstr()).into_owned());
    Some(short)
}

// ── Commit timestamps ────────────────────────────────────────────────────────

/// Return all commit timestamps (seconds since Unix epoch) across the entire history, newest first.
///
/// Equivalent to `git log --pretty=format:%at`.
pub fn git_commit_timestamps(root: &Path) -> Vec<u64> {
    let Some(repo) = open_repo(root) else {
        return Vec::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return Vec::new();
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return Vec::new();
    };
    walk.filter_map(|info| {
        let info = info.ok()?;
        info.commit_time.map(|t| t as u64)
    })
    .collect()
}

/// A single commit with its unix timestamp (for git_history.rs consumers).
pub struct CommitEntry {
    pub hash: String,
    pub timestamp: i64,
}

/// Return all commits with full hash and committer timestamp, oldest first.
///
/// Equivalent to `git log --format=%H%x00%at --reverse`.
pub fn git_log_timestamps(root: &Path) -> Result<Vec<CommitEntry>, String> {
    let repo = open_repo(root).ok_or("Not a git repository")?;
    let head_id = repo
        .head_id()
        .map_err(|e| format!("Failed to resolve HEAD: {e}"))?;
    let walk = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::OldestFirst,
        ))
        .all()
        .map_err(|e| format!("Failed to walk commits: {e}"))?;

    let mut commits = Vec::new();
    for info in walk {
        let info = info.map_err(|e| format!("Error walking commits: {e}"))?;
        let ts = info.commit_time.unwrap_or(0);
        commits.push(CommitEntry {
            hash: info.id.to_hex().to_string(),
            timestamp: ts,
        });
    }

    if commits.is_empty() {
        return Err("No commits found in git history".to_string());
    }
    Ok(commits)
}

// ── Ref resolution ───────────────────────────────────────────────────────────

/// Resolve a git ref (branch name, tag, `HEAD~N`, etc.) to a full SHA-1 hex string.
///
/// Equivalent to `git rev-parse --verify <ref>`.
pub fn resolve_ref(root: &Path, git_ref: &str) -> Result<String, String> {
    let repo = open_repo(root).ok_or("Not a git repository")?;
    let spec: &gix::bstr::BStr = git_ref.as_bytes().into();
    let id = repo
        .rev_parse_single(spec)
        .map_err(|e| format!("git ref '{git_ref}' not found: {e}"))?;
    Ok(id.to_hex().to_string())
}

/// Resolve the merge-base between `base` and HEAD.
///
/// Equivalent to `git merge-base <base> HEAD`, falling back to `base` as-is on error.
pub fn resolve_merge_base(root: &Path, base: &str) -> Result<String, String> {
    let repo = open_repo(root).ok_or("Not a git repository")?;
    let spec: &gix::bstr::BStr = base.as_bytes().into();
    let base_id = repo
        .rev_parse_single(spec)
        .map_err(|e| format!("git ref '{base}' not found: {e}"))?;
    let head_id = repo
        .head_id()
        .map_err(|e| format!("Failed to resolve HEAD: {e}"))?;
    // Try merge-base; if it fails, return the base ref directly (e.g. HEAD~3 style)
    match repo.merge_base(base_id.detach(), head_id.detach()) {
        Ok(mb) => Ok(mb.to_hex().to_string()),
        Err(_) => Ok(base_id.to_hex().to_string()),
    }
}

// ── File content at a ref ────────────────────────────────────────────────────

/// Read the content of `file_path` (repo-relative) at git ref `git_ref`.
///
/// Equivalent to `git show <git_ref>:<file_path>`.
pub fn git_show(root: &Path, git_ref: &str, file_path: &str) -> Option<String> {
    let repo = open_repo(root)?;
    let spec: &gix::bstr::BStr = git_ref.as_bytes().into();
    let id = repo.rev_parse_single(spec).ok()?;
    let commit = id.object().ok()?.into_commit();
    let tree = commit.tree().ok()?;
    let entry = tree.lookup_entry_by_path(file_path).ok()??;
    let blob = entry.object().ok()?.into_blob();
    String::from_utf8(blob.data.clone()).ok()
}

// ── Changed files in a diff ──────────────────────────────────────────────────

/// Status of a file in a diff (mirrors `skeleton_diff.rs` `FileStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFileStatus {
    Added,
    Deleted,
    Modified,
}

/// Return a list of (status, path) pairs for files changed between `base_ref` and HEAD.
///
/// Equivalent to `git diff --name-status <base_ref>`.
pub fn git_diff_name_status(
    root: &Path,
    base_ref: &str,
) -> Result<Vec<(DiffFileStatus, String)>, String> {
    let repo = open_repo(root).ok_or("Not a git repository")?;
    let spec: &gix::bstr::BStr = base_ref.as_bytes().into();
    let base_id = repo
        .rev_parse_single(spec)
        .map_err(|e| format!("git ref '{base_ref}' not found: {e}"))?;
    let head_id = repo
        .head_id()
        .map_err(|e| format!("Failed to resolve HEAD: {e}"))?;

    let base_commit = base_id.object().map_err(|e| e.to_string())?.into_commit();
    let head_commit = head_id.object().map_err(|e| e.to_string())?.into_commit();
    let base_tree = base_commit.tree().map_err(|e| e.to_string())?;
    let head_tree = head_commit.tree().map_err(|e| e.to_string())?;

    let changes = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)
        .map_err(|e| format!("diff failed: {e}"))?;

    let mut result = Vec::new();
    for change in changes {
        use gix::object::tree::diff::ChangeDetached;
        let (status, path) = match change {
            ChangeDetached::Addition { location, .. } => (
                DiffFileStatus::Added,
                String::from_utf8_lossy(&location).into_owned(),
            ),
            ChangeDetached::Deletion { location, .. } => (
                DiffFileStatus::Deleted,
                String::from_utf8_lossy(&location).into_owned(),
            ),
            ChangeDetached::Modification { location, .. } => (
                DiffFileStatus::Modified,
                String::from_utf8_lossy(&location).into_owned(),
            ),
            ChangeDetached::Rewrite {
                source_location, ..
            } => {
                // Treat rewrites (renames/copies) as modified
                (
                    DiffFileStatus::Modified,
                    String::from_utf8_lossy(&source_location).into_owned(),
                )
            }
        };
        result.push((status, path));
    }
    Ok(result)
}

// ── Tracked files ────────────────────────────────────────────────────────────

/// Return all file paths tracked by git (i.e. in the index).
///
/// Equivalent to `git ls-files`.
pub fn git_ls_files(root: &Path) -> Vec<String> {
    let Some(repo) = open_repo(root) else {
        return Vec::new();
    };
    let index = match repo.open_index() {
        Ok(idx) => idx,
        Err(_) => return Vec::new(),
    };
    use gix::bstr::ByteSlice;
    index
        .entries()
        .iter()
        .map(|entry| String::from_utf8_lossy(entry.path(&index).as_bytes()).into_owned())
        .collect()
}

// ── Remote URL ───────────────────────────────────────────────────────────────

/// Return the fetch URL of the `origin` remote, or `None`.
///
/// Equivalent to `git remote get-url origin`.
pub fn git_remote_origin_url(root: &Path) -> Option<String> {
    let repo = open_repo(root)?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url(gix::remote::Direction::Fetch)?;
    Some(url.to_bstring().to_string())
}

// ── Index status helpers ─────────────────────────────────────────────────────

/// Return true if `rel_dir` has uncommitted content changes (staged or unstaged) that
/// are NOT limited to the `doc_paths`.
///
/// Falls back to `false` on any error.
pub fn git_has_uncommitted_content_changes(
    root: &Path,
    rel_dir: &str,
    doc_paths: &[String],
) -> bool {
    let Some(repo) = open_repo(root) else {
        return false;
    };
    // Pass the directory path as a pattern so gix only checks files under it.
    let pattern: gix::bstr::BString = rel_dir.into();
    let patterns = if rel_dir == "." {
        Vec::new()
    } else {
        vec![pattern]
    };
    let platform = match repo.status(gix::progress::Discard) {
        Ok(p) => p.index_worktree_options_mut(|opts| {
            opts.dirwalk_options = None;
        }),
        Err(_) => return false,
    };
    let mut iter = match platform.into_index_worktree_iter(patterns) {
        Ok(it) => it,
        Err(_) => return false,
    };
    iter.any(|item| {
        let Ok(item) = item else { return false };
        use gix::bstr::ByteSlice;
        let rela = item.rela_path().to_str_lossy();
        !doc_paths.iter().any(|dp| dp.as_str() == rela.as_ref())
    })
}

/// Return true if `summary_path` has uncommitted changes (staged or unstaged).
pub fn git_summary_has_uncommitted_changes(root: &Path, summary_path: &str) -> bool {
    let Some(repo) = open_repo(root) else {
        return false;
    };
    let pattern: gix::bstr::BString = summary_path.into();
    let platform = match repo.status(gix::progress::Discard) {
        Ok(p) => p.index_worktree_options_mut(|opts| {
            opts.dirwalk_options = None;
        }),
        Err(_) => return false,
    };
    let mut iter = match platform.into_index_worktree_iter(vec![pattern]) {
        Ok(it) => it,
        Err(_) => return false,
    };
    iter.any(|item| item.is_ok())
}

// ── Commit count helpers (for stale-summary cache) ──────────────────────────

/// Return the commit hash of the last commit touching `rel_path`, or `None`.
///
/// Equivalent to `git log -1 --format=%H -- <rel_path>`.
/// Walks the commit history and diffs each commit against its first parent to find
/// commits that touched the given path.
pub fn git_last_commit_for_path(root: &Path, rel_path: &str) -> Option<String> {
    let repo = open_repo(root)?;
    let head_id = repo.head_id().ok()?;
    let walk = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .ok()?;

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
        let touches = changes.iter().any(|change| {
            use gix::object::tree::diff::ChangeDetached;
            let loc = match change {
                ChangeDetached::Addition { location, .. }
                | ChangeDetached::Deletion { location, .. }
                | ChangeDetached::Modification { location, .. } => location.as_slice(),
                ChangeDetached::Rewrite {
                    source_location, ..
                } => source_location.as_slice(),
            };
            loc == rel_path.as_bytes()
        });
        if touches {
            return Some(info.id.to_hex().to_string());
        }
    }
    None
}

/// Count commits touching `rel_dir` since `since_commit` (exclusive).
/// If `since_commit` is `None`, counts all commits touching `rel_dir`.
///
/// Walks the commit history and diffs each commit to find those that touched any
/// path under `rel_dir`.
pub fn git_commit_count_for_path(root: &Path, since_commit: Option<&str>, rel_dir: &str) -> usize {
    let Some(repo) = open_repo(root) else {
        return 0;
    };
    let Ok(head_id) = repo.head_id() else {
        return 0;
    };
    // Resolve the stop-commit (exclusive) if provided.
    let stop_id: Option<gix::hash::ObjectId> = since_commit.and_then(|h| {
        let spec: &gix::bstr::BStr = h.as_bytes().into();
        repo.rev_parse_single(spec).ok().map(|id| id.detach())
    });

    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return 0;
    };

    let mut count = 0;
    for info in walk {
        let Ok(info) = info else { continue };
        // Stop at since_commit (exclusive).
        if stop_id.is_some_and(|stop| info.id == stop) {
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
        let is_root = rel_dir == ".";
        let touches = if is_root {
            !changes.is_empty()
        } else {
            changes.iter().any(|change| {
                use gix::object::tree::diff::ChangeDetached;
                let loc = match change {
                    ChangeDetached::Addition { location, .. }
                    | ChangeDetached::Deletion { location, .. }
                    | ChangeDetached::Modification { location, .. } => location.as_slice(),
                    ChangeDetached::Rewrite {
                        source_location, ..
                    } => source_location.as_slice(),
                };
                loc.starts_with(rel_dir.as_bytes())
            })
        };
        if touches {
            count += 1;
        }
    }
    count
}

// ── Per-file churn stats ─────────────────────────────────────────────────────

/// One commit's contribution to a file's churn.
pub struct FileChurnEntry {
    /// Unix timestamp of the commit.
    pub timestamp: u64,
    /// Lines added in this commit.
    pub added: usize,
    /// Lines deleted in this commit.
    pub deleted: usize,
}

/// Walk all commits and produce per-file churn statistics.
///
/// Returns a map from file path to a list of per-commit churn entries.
/// Equivalent to parsing `git log --pretty=format:%at --numstat`.
///
/// NOTE: This is more expensive than the shell-out (requires O(N) tree diffs) but
/// eliminates the PATH dependency on the git binary.
pub fn git_file_churn_stats(root: &Path) -> std::collections::HashMap<String, Vec<FileChurnEntry>> {
    let Some(repo) = open_repo(root) else {
        return std::collections::HashMap::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return std::collections::HashMap::new();
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return std::collections::HashMap::new();
    };

    let mut stats: std::collections::HashMap<String, Vec<FileChurnEntry>> =
        std::collections::HashMap::new();

    for info in walk {
        let Ok(info) = info else { continue };
        let timestamp = info.commit_time.unwrap_or(0) as u64;
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

        for change in changes {
            use gix::object::tree::diff::ChangeDetached;
            let (path, old_id, new_id) = match &change {
                ChangeDetached::Modification {
                    location,
                    previous_id,
                    id,
                    ..
                } => (
                    String::from_utf8_lossy(location).into_owned(),
                    Some(*previous_id),
                    Some(*id),
                ),
                ChangeDetached::Addition { location, id, .. } => (
                    String::from_utf8_lossy(location).into_owned(),
                    None,
                    Some(*id),
                ),
                ChangeDetached::Deletion { location, id, .. } => (
                    String::from_utf8_lossy(location).into_owned(),
                    Some(*id),
                    None,
                ),
                ChangeDetached::Rewrite {
                    source_location,
                    id,
                    source_id,
                    ..
                } => (
                    String::from_utf8_lossy(source_location).into_owned(),
                    Some(*source_id),
                    Some(*id),
                ),
            };

            // Count lines by counting newlines in blob content
            let diff = count_diff_lines(&repo, old_id, new_id);

            stats.entry(path).or_default().push(FileChurnEntry {
                timestamp,
                added: diff.added,
                deleted: diff.deleted,
            });
        }
    }

    stats
}

/// Added/deleted line counts from `count_diff_lines`.
struct LineDiff {
    added: usize,
    deleted: usize,
}

/// Count added and deleted lines between two blob ids using a simple line count heuristic.
fn count_diff_lines(
    repo: &gix::Repository,
    old_id: Option<gix::hash::ObjectId>,
    new_id: Option<gix::hash::ObjectId>,
) -> LineDiff {
    let old_lines = old_id
        .and_then(|id| repo.find_object(id).ok())
        .map(|obj| count_lines(&obj.data))
        .unwrap_or(0);
    let new_lines = new_id
        .and_then(|id| repo.find_object(id).ok())
        .map(|obj| count_lines(&obj.data))
        .unwrap_or(0);

    // Simple heuristic: added = max(0, new - old), deleted = max(0, old - new)
    // This approximates `--numstat` without a full Myers diff.
    LineDiff {
        added: new_lines.saturating_sub(old_lines),
        deleted: old_lines.saturating_sub(new_lines),
    }
}

fn count_lines(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    data.iter().filter(|&&b| b == b'\n').count() + 1
}

// ── Author shortlog ──────────────────────────────────────────────────────────

/// One author's commit count in a repository.
pub struct AuthorCommitCount {
    pub name: String,
    pub email: String,
    pub commits: usize,
}

/// Return per-author commit counts for the repository.
///
/// Equivalent to `git shortlog -sne --all`.
pub fn git_author_commit_counts(root: &Path) -> Vec<AuthorCommitCount> {
    let Some(repo) = open_repo(root) else {
        return Vec::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return Vec::new();
    };
    let Ok(walk) = head_id.ancestors().all() else {
        return Vec::new();
    };

    // email -> (best_name, count)
    let mut map: HashMap<String, (String, usize)> = HashMap::new();

    for info in walk {
        let Ok(info) = info else { continue };
        let Ok(commit) = info.object() else { continue };
        let Ok(author) = commit.author() else {
            continue;
        };
        let email = String::from_utf8_lossy(author.email).into_owned();
        let name = String::from_utf8_lossy(author.name).into_owned();
        let entry = map.entry(email.clone()).or_insert((name.clone(), 0));
        // Keep the longest name variant
        if name.len() > entry.0.len() {
            entry.0 = name;
        }
        entry.1 += 1;
    }

    map.into_iter()
        .map(|(email, (name, commits))| AuthorCommitCount {
            name,
            email,
            commits,
        })
        .collect()
}

// ── Activity log (commit + author + file stats) ──────────────────────────────

/// One commit's data for the activity analysis.
pub struct ActivityCommit {
    pub timestamp: u64,
    pub author_email: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Walk all commits and collect activity data (timestamp, author, file stats).
///
/// Equivalent to parsing `git log --all --format=%H %at %ae --numstat`.
pub fn git_activity_commits(root: &Path) -> Vec<ActivityCommit> {
    let Some(repo) = open_repo(root) else {
        return Vec::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return Vec::new();
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return Vec::new();
    };

    let mut result = Vec::new();

    for info in walk {
        let Ok(info) = info else { continue };
        let timestamp = info.commit_time.unwrap_or(0) as u64;

        let Ok(commit) = info.object() else { continue };

        // Get author email
        let author_email = commit
            .author()
            .ok()
            .map(|a| String::from_utf8_lossy(a.email).into_owned())
            .unwrap_or_default();

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

        let files_changed = changes.len();
        let mut insertions = 0usize;
        let mut deletions = 0usize;

        for change in &changes {
            use gix::object::tree::diff::ChangeDetached;
            let (old_id, new_id) = match change {
                ChangeDetached::Modification {
                    previous_id, id, ..
                } => (Some(*previous_id), Some(*id)),
                ChangeDetached::Addition { id, .. } => (None, Some(*id)),
                ChangeDetached::Deletion { id, .. } => (Some(*id), None),
                ChangeDetached::Rewrite { source_id, id, .. } => (Some(*source_id), Some(*id)),
            };
            let old_lines = old_id
                .and_then(|id| repo.find_object(id).ok())
                .map(|obj| count_lines(&obj.data))
                .unwrap_or(0);
            let new_lines = new_id
                .and_then(|id| repo.find_object(id).ok())
                .map(|obj| count_lines(&obj.data))
                .unwrap_or(0);
            insertions += new_lines.saturating_sub(old_lines);
            deletions += old_lines.saturating_sub(new_lines);
        }

        result.push(ActivityCommit {
            timestamp,
            author_email,
            files_changed,
            insertions,
            deletions,
        });
    }

    result
}

// ── Per-commit changed file lists ────────────────────────────────────────────

/// Return a list of per-commit changed file paths, for temporal coupling analysis.
///
/// Each inner `Vec<String>` contains the paths of files changed in a single commit.
/// Equivalent to `git log --pretty=format:%x00 --name-only` (parsed).
pub fn git_per_commit_files(root: &Path) -> Vec<Vec<String>> {
    let Some(repo) = open_repo(root) else {
        return Vec::new();
    };
    let Ok(head_id) = repo.head_id() else {
        return Vec::new();
    };
    let Ok(walk) = head_id.ancestors().all() else {
        return Vec::new();
    };

    let mut result = Vec::new();

    for info in walk {
        let Ok(info) = info else { continue };
        let Ok(commit) = info.object() else { continue };
        let Ok(tree) = commit.tree() else { continue };

        // Diff against first parent (or empty tree for root commits)
        let parent_tree = info
            .parent_ids()
            .next()
            .and_then(|pid| pid.object().ok())
            .and_then(|obj| obj.into_commit().tree().ok());

        let changes = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let files: Vec<String> = changes
            .into_iter()
            .map(|change| {
                use gix::object::tree::diff::ChangeDetached;
                match change {
                    ChangeDetached::Addition { location, .. } => {
                        String::from_utf8_lossy(&location).into_owned()
                    }
                    ChangeDetached::Deletion { location, .. } => {
                        String::from_utf8_lossy(&location).into_owned()
                    }
                    ChangeDetached::Modification { location, .. } => {
                        String::from_utf8_lossy(&location).into_owned()
                    }
                    ChangeDetached::Rewrite {
                        source_location, ..
                    } => String::from_utf8_lossy(&source_location).into_owned(),
                }
            })
            .collect();

        if !files.is_empty() {
            result.push(files);
        }
    }

    result
}

// ── Date formatting ──────────────────────────────────────────────────────────

/// Format a Unix timestamp as `YYYY-MM-DD` without shelling out to `date`.
pub fn format_unix_date(ts: i64) -> String {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let days = ts / 86400;
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
    format!("{y:04}-{m:02}-{d:02}")
}

// ── Worktree helpers (still shell-out, write ops not supported by gix) ───────

/// Resolve a git ref to a full commit hash via gix.
pub fn resolve_ref_shellout(root: &Path, git_ref: &str) -> Result<String, String> {
    resolve_ref(root, git_ref)
}

/// Create a detached worktree at `hash`, run `callback`, then remove the worktree.
///
/// The worktree is always removed after `callback` completes, even if it returns an error.
/// (gix does not support worktree add/remove; this remains a shell-out.)
pub fn run_in_worktree<T, F>(root: &Path, hash: &str, callback: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let short = &hash[..7.min(hash.len())];
    let worktree_name = format!("normalize-wt-{short}");
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    if worktree_path.exists() {
        let _ = std::process::Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
    }

    let add_output = std::process::Command::new("git")
        .args(["worktree", "add", "--detach", &worktree_str, hash])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to create worktree: {e}"))?;

    if !add_output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_output.stderr).trim()
        ));
    }

    let result = callback(&worktree_path);

    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

    result
}
