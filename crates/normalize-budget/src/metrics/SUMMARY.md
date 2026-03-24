# normalize-budget/src/metrics

Diff metric implementations for the budget system.

- `mod.rs` — `DiffMetric` trait: `measure_diff` returns `Vec<DiffMeasurement>`; `DiffMeasurement` struct with `key`, `added`, `removed` fields
- `lines.rs` — `LineDeltaMetric`: line-level diff via `git diff --numstat`
- `functions.rs` — `FunctionDeltaMetric`: functions/methods added or removed (worktree-based symbol diff); also contains `create_worktree`/`remove_worktree`/`symbol_diff` utilities
- `classes.rs` — `ClassDeltaMetric`: classes/structs/types added or removed (uses `symbol_diff` from `functions.rs`)
- `modules.rs` — `ModuleDeltaMetric`: files added or removed via `git diff --name-status`
- `todos.rs` — `TodoDeltaMetric`: TODO/FIXME comments added or removed from diff
- `complexity_delta.rs` — `ComplexityDeltaMetric`: complexity increase/decrease per function (worktree-based)
- `dependencies.rs` — `DependencyDeltaMetric`: dependency entries added or removed from manifest files
