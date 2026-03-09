//! Git history helpers — shared worktree infrastructure for diff and trend analysis.

use std::path::Path;
use std::process::Command;

/// A single commit with its unix timestamp.
pub struct CommitInfo {
    pub hash: String,
    pub timestamp: i64,
}

/// Get all commits with timestamps, oldest first.
pub fn git_log_timestamps(root: &Path) -> Result<Vec<CommitInfo>, String> {
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
    let output = Command::new("git")
        .args(["rev-parse", "--verify", git_ref])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git rev-parse: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git ref '{}' not found: {}",
            git_ref,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Create a detached worktree at `hash`, run `callback`, then remove the worktree.
///
/// The worktree is always removed after `callback` completes, even if it returns an error.
pub fn run_in_worktree<T, F>(root: &Path, hash: &str, callback: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let short = &hash[..7.min(hash.len())];
    let worktree_name = format!("normalize-wt-{short}");
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    // Clean up any stale worktree
    if worktree_path.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
    }

    // Create worktree
    let add_output = Command::new("git")
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

    // Run callback, always clean up after
    let result = callback(&worktree_path);

    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

    result
}

/// Format a unix timestamp as YYYY-MM-DD.
pub fn format_unix_date(ts: i64) -> String {
    let output = Command::new("date")
        .args(["-d", &format!("@{ts}"), "+%Y-%m-%d"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => format!("{ts}"),
    }
}
