# normalize-budget/src/metrics

Diff metric implementations for the budget system.

- `mod.rs` — `DiffMetric` trait: `measure_diff` returns `(key, added, removed)` triples
- `lines.rs` — Line-level diff via `git diff --numstat`
- `functions.rs` — Functions/methods added or removed (worktree-based symbol diff); also contains `create_worktree`/`remove_worktree`/`symbol_diff` utilities
- `classes.rs` — Classes/structs/types added or removed (uses `symbol_diff` from `functions.rs`)
- `modules.rs` — Files added or removed via `git diff --name-status`
- `todos.rs` — TODO/FIXME comments added or removed from diff
- `complexity_delta.rs` — Complexity increase/decrease per function (worktree-based)
- `dependencies.rs` — Dependency entries added or removed from manifest files
