# Shadow Git Design

Auto-track edits made via `moss edit` for undo/redo capability.

## Problem

When `moss edit` modifies files, there's no easy way to undo changes. Users must rely on git or manual backups.

## Solution

Maintain a hidden git repository (`.moss/shadow/`) that automatically commits after each `moss edit` operation, preserving full edit history as a tree.

## Core Features

### Automatic Tracking
- Every `moss edit` operation creates a shadow commit
- Commit message includes: operation, target, timestamp, optional user message
- Only tracks files modified by moss, not external changes

### Edit Messages
```bash
moss edit src/foo.rs/bar delete --message "Removing deprecated function"
moss edit src/foo.rs/bar delete --reason "Removing deprecated function"  # alias
```

Optional `--message` (or `--reason`) flag attaches a description to the edit, displayed in history and undo output.

### Undo/Redo
```bash
moss edit --undo              # Revert last moss edit, prints what was undone
moss edit --undo 3            # Revert last 3 edits, prints summary of each
moss edit --redo              # Re-apply last undone edit
moss edit --history           # Show recent moss edits
moss edit --history src/foo.rs  # Show edits for specific file
```

Undo output includes:
- Files changed
- Edit descriptions (from `--message` if provided)
- Operation type and target

### Configuration
```toml
[shadow]
enabled = true                # Default: true
retention_days = 30           # Auto-cleanup old commits
warn_on_delete = true         # Confirm before deleting symbols
```

## Architecture

### Tree Structure (Not Linear)

Shadow history is a **tree**, not a linear history:
- Undo moves HEAD backward but doesn't destroy commits
- New edits after undo create a branch (fork in history)
- All edits preserved (can return to any previous state)
- Branches can be pruned for security (remove sensitive content from history)

```
         A -- B -- C -- D  (original history, still exists)
              \
               E -- F      (new branch: after undoing to B, made edits E, F)
                    ^
                   HEAD
```

Undo/redo mechanics:
- `--undo`: moves HEAD to parent commit, applies reverse patch to user files
- `--undo N`: undoes N commits in sequence
- `--redo`: moves HEAD to child commit, applies forward patch
  - If multiple children exist (branch point), prompts user or requires `--redo <ref>`
- After undo, new edits create a branch from current HEAD
- Original commits (like D above) still exist, reachable via `--history --all`

**Conflict handling**: If reverse patch doesn't apply (file was modified externally):
- Abort undo and report conflict
- User must resolve manually (e.g., discard external changes or use `moss edit --force-undo`)
- `--force-undo` overwrites file with shadow's known state (destructive)

**Branch navigation**: To restore a different branch:
```bash
moss edit --goto <ref>        # Move HEAD to ref, restore file to that state
moss edit --goto 2            # Go to commit 2 (by number from --history)
```

### Directory Structure
```
.moss/
  shadow/
    .git/                     # Shadow repository
      refs/
        heads/
          main                # Current position in edit tree
        files/                # Per-file branch heads (Phase 2)
          src%2Ffoo.rs        # HEAD for src/foo.rs edits (URL-encoded path)
    worktree/                 # Working copy of tracked files
```

The shadow repo tracks files in a separate worktree, not the user's actual files. On each `moss edit`:
1. Copy current file state to worktree (captures "before")
2. Apply edit to user's file
3. Copy new file state to worktree (captures "after")
4. Commit the change

**Initial state**: The shadow repo is created on first `moss edit`. The "initial state" commit (commit 0) contains the file's state before that first edit. Files not yet edited have no shadow history.

### Shadow Commit Format

Commit message (structured for parsing):
```
moss edit: delete src/foo.rs/deprecated_fn

Message: Removing deprecated function
Operation: delete
Target: src/foo.rs/deprecated_fn
Files: src/foo.rs
```

Git stores the diff separately. Timestamp comes from git commit metadata.

### Multi-File Edits

Some operations may touch multiple files (future: cross-file refactors like `moss move`):
- Shadow commit is atomic: all files in one commit
- Undo reverts all files atomically
- Per-file history (Phase 2) tracks each file's HEAD independently
  - `--history src/foo.rs` shows only commits affecting that file
  - `--undo --file src/foo.rs` reverts only that file's changes (creates partial undo commit)

### Branch Pruning (Security)

If sensitive content was accidentally committed:
```bash
moss edit --prune <commit-range>  # Remove commits from shadow history
moss edit --prune-file src/secrets.rs  # Remove all history for a file
```

Uses `git filter-branch` or similar under the hood. Important for:
- Removing accidentally committed secrets
- Cleaning up after experiments
- Reducing repo size

## Design Decisions

### D1: Tree structure over linear
- **Decision**: Preserve all history as tree
- **Rationale**: Undo shouldn't destroy information; users might want to return to undone state
- **Trade-off**: More disk usage, but git handles this well

### D2: Per-file history (Phase 2)
- **Decision**: Add per-file branches immediately after basic implementation
- **Rationale**: Users often want to undo edits to specific files without affecting others
- **Implementation**: `refs/files/<path>` tracks per-file HEAD

### D3: Storage format
- **Decision**: Use git
- **Rationale**: Delta compression, familiar tooling, handles trees naturally

### D4: External changes
- **Decision**: Re-sync by reading current file state before commit
- **Rationale**: Shadow tracks moss edits, not manual edits; patch may fail if file diverged

### D5: Relationship to real git
- **Decision**: Fully independent
- **Rationale**: Shadow is for "oops" recovery, not version control; don't interfere with user's git workflow

## Implementation Plan

### Phase 1: Basic Infrastructure
- [ ] Create `.moss/shadow/` git repo on first `moss edit`
- [ ] Commit file state before each edit
- [ ] `--message`/`--reason` flag for edit descriptions
- [ ] `--history` to list recent edits

### Phase 2: Undo/Redo + Per-File
- [ ] `--undo` applies reverse patch, moves HEAD backward, prints summary
- [ ] `--undo N` reverts N edits in sequence
- [ ] `--redo` moves HEAD forward, applies patch (error if at branch point)
- [ ] `--goto <ref>` jumps to arbitrary commit
- [ ] Conflict detection and `--force-undo` for external modifications
- [ ] `--history --all` shows full tree structure
- [ ] Per-file branches (`refs/files/<path>`) and `--history <file>`

### Phase 3: Security + Polish
- [ ] `--prune` for removing commits/branches
- [ ] Retention policy / auto-cleanup (only prunes merged branches)
- [ ] `warn_on_delete` confirmation

## Risks

1. **Disk usage**: Tree structure preserves everything
   - Mitigation: Retention policy prunes old merged branches, `--prune` for manual cleanup, git gc

2. **Performance**: Git operations add latency
   - Mitigation: Commits are small; consider async commits for non-blocking edits

3. **Complexity**: Tree navigation
   - Mitigation: Simple undo/redo for common case; tree visible only via `--history --all`

## Example Session

```bash
$ moss edit src/foo.rs/old_fn delete --message "Cleanup"
delete: old_fn in src/foo.rs

$ moss edit src/foo.rs/helper rename new_helper
rename: helper -> new_helper in src/foo.rs

$ moss edit --history
  2. [HEAD] rename: helper -> new_helper in src/foo.rs
  1. delete: old_fn in src/foo.rs "Cleanup"

$ moss edit --undo 2
Undoing 2 edits:
  [2] rename: helper -> new_helper
  [1] delete: old_fn "Cleanup"
Files restored: src/foo.rs
HEAD now at: (initial state)

$ moss edit src/foo.rs/new_fn insert "fn new_fn() {}"
insert: new_fn in src/foo.rs
(created branch from initial state)

$ moss edit --history --all
  * 3. [HEAD] insert: new_fn in src/foo.rs
  |
  | 2. rename: helper -> new_helper in src/foo.rs
  | 1. delete: old_fn in src/foo.rs "Cleanup"
  |/
  0. (initial state)

$ moss edit --redo
error: No forward history from current position.
hint: Use --history --all to see other branches.
```
