# src

Source for the `normalize-ratchet` crate.

- `lib.rs` — public API: `Metric` trait, `MetricFactory` type alias (`fn(&Path) -> Vec<Box<dyn Metric>>`), `default_metrics()` factory, re-exports from `baseline` and `metrics`
- `baseline.rs` — baseline file format (`.normalize/ratchet.json`), `Aggregate` enum, `BaselineEntry`, `BaselineFile`, `RatchetConfig`, `RatchetConfigMetric`, aggregation logic, load/save helpers
- `metrics/` — metric implementations (complexity, call-complexity, line-count, function-count, class-count, comment-line-count)
- `service.rs` — CLI service (`normalize ratchet`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; also `build_ratchet_diagnostics()` for native rules integration. Behind the `cli` feature.
