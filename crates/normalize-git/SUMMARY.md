# normalize-git

Pure-Rust read-only git operations using `gix` — no PATH dependency on the git binary.

Extracted from the main `normalize` crate (`commands/analyze/git_utils.rs`) and consolidated with duplicate implementations previously copied verbatim into `normalize-budget`, `normalize-ratchet`, and `normalize-semantic`.

## Contents

- `Cargo.toml` — crate manifest; depends on `gix` (workspace) and `anyhow`
- `src/lib.rs` — the full public API

## Public API (src/lib.rs)

Repository: `open_repo`. HEAD/refs: `git_head`, `git_head_branch`, `resolve_ref`, `resolve_merge_base`, `resolve_ref_shellout`. Commits: `CommitEntry`, `git_log_timestamps`, `git_commit_timestamps`, `git_last_commit_for_path`, `git_commit_count_for_path`. Blobs/trees: `read_blob_text`, `read_blob_bytes`, `git_show`, `walk_tree_at_ref`. Diff: `DiffFileStatus`, `git_diff_name_status`, `FileChangeKind`, `FileChange`, `diff_base_to_head`. Index: `git_ls_files`, `git_has_uncommitted_content_changes`, `git_summary_has_uncommitted_changes`. Churn/activity: `FileChurnEntry`, `git_file_churn_stats`, `AuthorCommitCount`, `git_author_commit_counts`, `ActivityCommit`, `git_activity_commits`, `git_per_commit_files`. Remote/date: `git_remote_origin_url`, `format_unix_date`. Worktree: `run_in_worktree`.

## Dependents

- `normalize` (main crate) — re-exported from `commands/analyze/git_utils.rs`
- `normalize-budget` — `git_ops.rs` re-exports read helpers
- `normalize-ratchet` — `git_ops.rs` re-exports read helpers
- `normalize-semantic` — `git_staleness.rs` uses `open_repo` directly
