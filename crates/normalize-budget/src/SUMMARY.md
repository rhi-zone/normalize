# normalize-budget/src

Diff-based budget system implementation.

- `lib.rs` — `DiffMetricFactory` type alias, `BudgetError`, `default_diff_metrics()`, re-exports
- `budget.rs` — `BudgetEntry`, `BudgetFile`, `BudgetLimits` (fields: `max_added`, `max_removed`, `max_total`, `max_net`), `BudgetConfig`, file I/O functions
- `error.rs` — `BudgetError` enum (thiserror-based)
- `metrics/` — `DiffMetric` trait, `DiffMeasurement` struct, and 7 implementations
- `service.rs` — CLI service (`BudgetService`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; also `build_budget_report` for native rules integration
