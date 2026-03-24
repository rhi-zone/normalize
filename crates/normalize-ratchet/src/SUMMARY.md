# src

Source for the `normalize-ratchet` crate.

- `lib.rs` — public API: re-exports `Metric` and `MetricFactory` from `normalize-metrics`, `default_metrics()` factory, re-exports from `baseline` and `metrics`; also re-exports `ratchet_path` for symmetry with `normalize-budget`'s `budget_path`
- `baseline.rs` — baseline file format (`.normalize/ratchet.json`), re-exports `Aggregate`/`aggregate` from `normalize-metrics`, `BaselineEntry`, `BaselineFile`, `RatchetConfig`, `RatchetConfigMetric`, load/save helpers; `load_baseline`/`save_baseline` free functions deprecated in favour of `BaselineFile::load`/`BaselineFile::save`
- `metrics/` — metric implementations (complexity, call-complexity, line-count, function-count, class-count, comment-line-count); `Metric` re-exported from `normalize-metrics`
- `service.rs` — CLI service (`normalize ratchet`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; report types `MeasureReport`, `AddReport`, `ShowEntry` use `Aggregate` (not `String`) for the `aggregate` field; `measure()` is the canonical public function; `do_measure()` is a deprecated alias for `measure()`; also `build_ratchet_report()` / `build_ratchet_diagnostics()` (deprecated since 0.2.0, use `build_ratchet_report`) for native rules integration. Report types: `MeasureReport`, `AddReport`, `RemoveReport`, `CheckReport`, `UpdateReport`, `ShowReport`. Behind the `cli` feature. Uses `WorktreeGuard` (RAII) for safe worktree cleanup in ref-based check and measure. `RatchetError` used for metric-not-found and measurement-failed error paths.
- `error.rs` — `RatchetError` enum (thiserror) with variants for metric-not-found, baseline IO/parse, measurement failures (field: `reason`)
