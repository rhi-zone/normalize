//! Git history helpers — shared worktree infrastructure for diff and trend analysis.

use std::path::Path;

/// A single commit with its unix timestamp.
pub struct CommitInfo {
    pub hash: String,
    pub timestamp: i64,
}

/// Get all commits with timestamps, oldest first.
pub fn git_log_timestamps(root: &Path) -> Result<Vec<CommitInfo>, String> {
    let entries = super::git_utils::git_log_timestamps(root)?;
    Ok(entries
        .into_iter()
        .map(|e| CommitInfo {
            hash: e.hash,
            timestamp: e.timestamp,
        })
        .collect())
}

/// Pick N commits at regular time intervals from the commit list.
pub fn select_snapshots(commits: &[CommitInfo], n: usize) -> Vec<&CommitInfo> {
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

/// Resolve a git ref (branch name, tag, short hash, HEAD~N, etc.) to a full commit hash.
pub fn resolve_ref(root: &Path, git_ref: &str) -> Result<String, String> {
    super::git_utils::resolve_ref(root, git_ref)
}

/// Create a detached worktree at `hash`, run `callback`, then remove the worktree.
///
/// The worktree is always removed after `callback` completes, even if it returns an error.
/// Write operations (worktree add/remove) are not supported by gix — kept as shell-outs.
pub fn run_in_worktree<T, F>(root: &Path, hash: &str, callback: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    super::git_utils::run_in_worktree(root, hash, callback)
}

/// Format a unix timestamp as YYYY-MM-DD.
pub fn format_unix_date(ts: i64) -> String {
    super::git_utils::format_unix_date(ts)
}
