# normalize-budget

Diff-based budget system for normalize: tracks how much a codebase is allowed to change across a set of metrics (lines added/removed, function count deltas, etc.). Published as a standalone crate. The `cli` feature flag exposes `BudgetService` for use in the normalize binary's service layer. The mutating subcommands (`add`, `update`, `remove`) accept `--dry-run` to preview config changes without writing `budget.json`.

`budget measure` and `budget add` accept `--diff-ref <ref>` (the git ref to measure growth against; renamed from `--base-ref` in the server-less 0.6 upgrade).

- `Cargo.toml` — crate manifest; `cli` feature gates `server-less` dependency
- `src/` — implementation (see `src/SUMMARY.md`): `budget.rs`, `service.rs`, `git_ops.rs`, `error.rs`, and per-metric calculators under `src/metrics/` (`lines`, `functions`, `classes`, `modules`, `dependencies`, `complexity_delta`, `todos`)
