# src

Source for the `normalize-ratchet` crate.

- `lib.rs` — public API: re-exports `Metric` and `MetricFactory` from `normalize-metrics`, `default_metrics()` factory, re-exports from `baseline` and `metrics`
- `baseline.rs` — baseline file format (`.normalize/ratchet.json`), re-exports `Aggregate`/`aggregate` from `normalize-metrics`, `BaselineEntry`, `BaselineFile`, `RatchetConfig`, `RatchetConfigMetric`, load/save helpers
- `metrics/` — metric implementations (complexity, call-complexity, line-count, function-count, class-count, comment-line-count); `Metric` re-exported from `normalize-metrics`
- `service.rs` — CLI service (`normalize ratchet`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; also `build_ratchet_report()` / `build_ratchet_diagnostics()` for native rules integration. Report types: `MeasureReport`, `AddReport`, `RemoveReport`, `CheckReport`, `UpdateReport`, `ShowReport`. Behind the `cli` feature.
- `error.rs` — `RatchetError` enum (thiserror) with variants for metric-not-found, baseline IO/parse, measurement failures.
