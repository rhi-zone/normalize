//! Git-based staleness computation for symbol embeddings.
//!
//! Staleness reflects how much time has passed (in commits) since a file was
//! last updated. For symbol chunks (where doc and code are colocated), the
//! approximation is:
//!
//! 1. Find `last_doc_commit` — the most recent commit that touched the file.
//! 2. Count commits to the file since `last_doc_commit` (exclusive). Because
//!    `last_doc_commit` is itself the last file touch, this counts repo-wide
//!    commits that did NOT touch the file — i.e. how many commits have elapsed
//!    with no update to this file.
//! 3. `staleness = min(1.0, commits_since_doc_update as f64 / 50.0)`
//!
//! Gives 0.0 for files updated in the most recent commit, approaching 1.0 after
//! 50+ commits have landed without touching the file.
//!
//! ## Performance
//!
//! Use [`compute_staleness_batch`] to amortize the git walk across all files in
//! a populate run. The function deduplicates paths and walks history once per
//! unique file.

use std::collections::HashMap;
use std::path::Path;

/// Compute staleness scores for a batch of file paths.
///
/// Returns a `HashMap<file_path, staleness>` where staleness is in `[0.0, 1.0]`.
/// Each unique file path triggers one git history walk; paths are deduplicated
/// internally so callers can pass one entry per symbol without penalty.
///
/// Degrades gracefully to `0.0` if the repository cannot be opened.
pub fn compute_staleness_batch(
    root: &Path,
    file_paths: &[impl AsRef<str>],
) -> HashMap<String, f64> {
    let mut result: HashMap<String, f64> = HashMap::new();

    let Some(repo) = open_repo(root) else {
        for p in file_paths {
            result.insert(p.as_ref().to_string(), 0.0);
        }
        return result;
    };

    // Deduplicate so each file's history is walked at most once.
    let mut seen = std::collections::HashSet::new();
    let unique_paths: Vec<&str> = file_paths
        .iter()
        .map(|p| p.as_ref())
        .filter(|p| seen.insert(*p))
        .collect();

    for rel_path in unique_paths {
        let staleness = compute_staleness_for_file(&repo, rel_path);
        result.insert(rel_path.to_string(), staleness);
    }

    result
}

/// Compute the staleness score for a single file.
///
/// Algorithm:
/// 1. Walk commit history (newest first), finding the most recent commit that
///    touched `rel_path` — this is `last_doc_commit`.
/// 2. Count all subsequent (newer) commits that did NOT touch the file —
///    these are `commits_since_doc_update`.
/// 3. `staleness = min(1.0, commits_since_doc_update / 50.0)`.
fn compute_staleness_for_file(repo: &gix::Repository, rel_path: &str) -> f64 {
    let Ok(head_id) = repo.head_id() else {
        return 0.0;
    };
    let Ok(walk) = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    else {
        return 0.0;
    };

    // Walk commits newest-first.
    // `commits_before_last_touch` counts commits traversed before we find the
    // last commit that touched rel_path. Those are commits that have elapsed
    // since the file was last updated.
    let mut commits_before_last_touch = 0usize;
    let mut found = false;

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
            found = true;
            break;
        }

        commits_before_last_touch += 1;
    }

    if !found {
        // No commit ever touched this file — treat as fresh (likely untracked).
        return 0.0;
    }

    (commits_before_last_touch as f64 / 50.0).min(1.0)
}

fn open_repo(path: &Path) -> Option<gix::Repository> {
    gix::discover(path).ok()
}
