# normalize-budget/src

Diff-based budget system implementation.

- `lib.rs` — `DiffMetricFactory` type alias, `BudgetError`, `default_diff_metrics()`, re-exports
- `budget.rs` — `BudgetEntry`, `BudgetFile` (with `load`/`save` methods), `BudgetLimits` (fields: `max_added`, `max_removed`, `max_total`, `max_net`), `BudgetConfig`, path helper `budget_path()`; `load_budget`/`save_budget` free functions are deprecated in favour of `BudgetFile::load`/`BudgetFile::save`
- `error.rs` — `BudgetError` enum (thiserror-based); `MeasurementFailed.reason` (unified with ratchet)
- `metrics/` — `DiffMetric` trait, `DiffMeasurement` struct, and 7 implementations; `WorktreeGuard` RAII type in `functions.rs`
- `service.rs` — CLI service (`BudgetService`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; report types `MeasureReport`, `ShowEntry`, `CheckEntry` use `Aggregate` (not `String`) for the `aggregate` field; `build_budget_report` for native rules integration; `build_budget_diagnostics` is a deprecated alias for `build_budget_report`
