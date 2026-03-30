# normalize-budget/src/metrics

Diff metric implementations for the budget system. All metrics use gix (pure-Rust git) — no `git` binary in `$PATH` required.

- `mod.rs` — `DiffMetric` trait: `measure_diff` returns `Vec<DiffMeasurement>`; `DiffMeasurement` struct with `key`, `added`, `removed` fields
- `lines.rs` — `LineDeltaMetric`: line-level diff by counting newlines in base/HEAD blobs via gix
- `functions.rs` — `FunctionDeltaMetric`: functions/methods added or removed; reads base tree blobs via `git_ops::walk_tree_at_ref`, working tree from disk; exports `symbol_diff` used by `classes.rs`
- `classes.rs` — `ClassDeltaMetric`: classes/structs/types added or removed (uses `symbol_diff` from `functions.rs`)
- `modules.rs` — `ModuleDeltaMetric`: files added or removed via gix `diff_tree_to_tree`
- `todos.rs` — `TodoDeltaMetric`: TODO/FIXME comments added or removed by counting matching lines in base/HEAD blobs
- `complexity_delta.rs` — `ComplexityDeltaMetric`: complexity increase/decrease per function; reads base tree blobs via `git_ops::walk_tree_at_ref`, working tree from disk
- `dependencies.rs` — `DependencyDeltaMetric`: dependency entries added or removed from manifest files via gix blob comparison
