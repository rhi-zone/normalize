# normalize-budget/src

Diff-based budget system implementation.

- `lib.rs` — `DiffMetricFactory` type alias, `default_diff_metrics()`, re-exports
- `budget.rs` — `BudgetEntry`, `BudgetFile`, `BudgetLimits`, `BudgetConfig`, file I/O functions
- `metrics/` — `DiffMetric` trait and 7 implementations (lines, functions, classes, modules, todos, complexity-delta, dependencies)
- `service.rs` — CLI service (`BudgetService`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; also `build_budget_diagnostics` for native rules integration
