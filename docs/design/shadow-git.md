# Shadow Git Design

Auto-track edits made via `moss edit` for undo/redo capability.

## Problem

When `moss edit` modifies files, there's no easy way to undo changes. Users must rely on git or manual backups.

## Solution

Maintain a hidden git repository (`.moss/shadow/`) that automatically commits after each `moss edit` operation.

## Core Features

### Automatic Tracking
- Every `moss edit` operation creates a shadow commit
- Commit message includes: operation, target, timestamp
- Only tracks files modified by moss, not external changes

### Undo/Redo
```bash
moss edit --undo              # Revert last moss edit
moss edit --undo 3            # Revert last 3 edits
moss edit --redo              # Re-apply last undone edit
moss edit --history           # Show recent moss edits
```

### Configuration
```toml
[shadow]
enabled = true                # Default: true
retention_days = 30           # Auto-cleanup old commits
warn_on_delete = true         # Confirm before deleting symbols
```

## Architecture

### Directory Structure
```
.moss/
  shadow/
    .git/                     # Shadow repository
    HEAD                      # Current position (for undo/redo)
    refs/
      edits/                  # Branch per file? Or single linear history?
```

### Shadow Commit Format
```
moss edit: delete src/foo.rs/deprecated_fn

Operation: delete
Target: src/foo.rs/deprecated_fn
Timestamp: 2025-01-01T12:00:00Z
---
[patch content]
```

## Design Questions

### Q1: Single history or per-file?
- **Single linear history**: Simpler, but undo affects all files
- **Per-file branches**: More granular, but complex to manage
- **Recommendation**: Start with single history, add per-file later if needed

### Q2: Storage format?
- **Full git repo**: Uses git's delta compression, familiar tooling
- **Custom format**: More control, but reinvents wheel
- **Recommendation**: Use git - it's designed for this

### Q3: What about external changes?
- Shadow git only tracks moss edits
- If user makes manual changes, shadow history diverges from actual file state
- Options:
  - A) Detect and warn on divergence
  - B) Re-sync shadow on next moss edit
  - C) Ignore - undo applies patch, may fail if file changed
- **Recommendation**: Option B - re-sync by reading current file state before commit

### Q4: Relationship to real git?
- Shadow git is independent - doesn't interfere with user's git
- After `moss edit --undo`, user still needs to commit/discard in real git
- Shadow is for "oops, wrong edit" recovery, not version control

### Q5: Multi-file edits?
- Some operations touch multiple files (future: cross-file refactors)
- Shadow commit should be atomic for multi-file edits
- Undo should revert all files in the edit atomically

## Implementation Plan

### Phase 1: Basic Infrastructure
- [ ] Create `.moss/shadow/` git repo on first `moss edit`
- [ ] Commit file state before each edit
- [ ] `--history` to list recent edits

### Phase 2: Undo/Redo
- [ ] `--undo` applies reverse patch
- [ ] `--redo` re-applies forward patch
- [ ] Handle conflicts gracefully

### Phase 3: Polish
- [ ] Retention policy / auto-cleanup
- [ ] `warn_on_delete` confirmation
- [ ] Per-file history view

## Risks

1. **Disk usage**: Shadow repo grows over time
   - Mitigation: Retention policy, git gc

2. **Performance**: Git operations add latency to edits
   - Mitigation: Commits are small (single file patches)

3. **Complexity**: Another git repo to manage
   - Mitigation: Fully automatic, user never interacts directly

## Alternatives Considered

### Backup files (foo.rs.bak)
- Simple but clutters workspace
- No history, just last version

### SQLite changelog
- Flexible but custom format
- No tooling for inspection

### Integration with real git
- Interferes with user's workflow
- Requires git to be initialized

## Decision

Use shadow git: minimal complexity, leverages git's strengths, fully isolated from user's workflow.
