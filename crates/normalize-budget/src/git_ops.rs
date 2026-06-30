//! Git operations for the budget crate — thin re-exports from `normalize_git`.
//!
//! Low-level gix helpers (`read_blob_text`, `read_blob_bytes`, `walk_tree_at_ref`,
//! `diff_base_to_head`, `FileChangeKind`, `FileChange`) are re-exported directly.
//!
//! `open_repo` is wrapped to return `anyhow::Result` (matching the existing call
//! sites in this crate that propagate errors via `?`).
pub use normalize_git::{
    FileChange, FileChangeKind, diff_base_to_head, read_blob_bytes, read_blob_text,
    walk_tree_at_ref,
};

/// Open the git repository at or containing `path`.
///
/// Thin wrapper around `normalize_git::open_repo` that converts `None` into an
/// `anyhow::Error` for callers that need to propagate the error.
pub fn open_repo(path: &std::path::Path) -> anyhow::Result<gix::Repository> {
    normalize_git::open_repo(path)
        .ok_or_else(|| anyhow::anyhow!("not a git repository: {}", path.display()))
}
