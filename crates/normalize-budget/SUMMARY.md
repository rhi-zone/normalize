# normalize-budget

Diff-based budget system for normalize: tracks how much a codebase is allowed to change across a set of metrics (lines added/removed, function count deltas, etc.). Published as a standalone crate. The `cli` feature flag exposes `BudgetService` for use in the normalize binary's service layer.

- `Cargo.toml` — crate manifest; `cli` feature gates `server-less` dependency
- `src/` — implementation (see `src/SUMMARY.md`): `budget.rs`, `service.rs`, `git_ops.rs`, `error.rs`, and per-metric calculators under `src/metrics/` (`lines`, `functions`, `classes`, `modules`, `dependencies`, `complexity_delta`, `todos`)
