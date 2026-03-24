# normalize-ratchet

Metric regression-tracking ("ratchet") system for normalize.

Each baseline entry is a `(path, metric, aggregation) → value` triple stored in `.normalize/ratchet.json`. The system measures current values, compares them to baselines, and reports regressions.

## Key Concepts

- **Path addressing**: uses `file/Parent/function` or `file/function` format (same as `normalize view`). Can be a directory, file, or symbol.
- **Aggregation**: `mean | median | max | min | sum | count` — configurable per entry, with defaults in config.
- **Higher-is-worse vs lower-is-worse**: complexity metrics use higher-is-worse; coverage metrics use lower-is-worse.

## Structure

- `src/lib.rs` — public API: `Metric` trait, `MetricFactory` type alias, `default_metrics()` factory
- `src/baseline.rs` — baseline file format (`.normalize/ratchet.json`), `Aggregate` enum, `RatchetConfig`, aggregation logic
- `src/metrics/` — metric implementations (complexity, call-complexity, line-count, function-count, class-count, comment-line-count)
- `src/error.rs` — `RatchetError` enum with thiserror for metric-not-found, baseline IO/parse, measurement failures
- `src/service.rs` — CLI service (`normalize ratchet`) with `measure`, `add`, `check`, `update`, `show`, `remove` commands; report types renamed `*Report` (was `MeasureResult`, `AddResult`, `RemoveResult`); `build_ratchet_report()` for native rules integration

## Metrics

1. **complexity** — cyclomatic complexity per function (`file/Parent/fn → f64`)
2. **call-complexity** — transitive cyclomatic complexity via BFS (`file/Parent/fn → f64`)
3. **line-count** — lines per file (`file → f64`)
4. **function-count** — function count per file (`file → f64`)
5. **class-count** — class/struct/interface count per file (`file → f64`)
6. **comment-line-count** — comment lines per file (`file → f64`)

## Integration

- `cli` feature gates `service.rs` (server-less `#[cli]` macro, schemars derives)
- `RatchetConfig` is included in `NormalizeConfig` via `#[param(nested, serde)]`
- `normalize-native-rules` uses `MetricFactory` (not behind `cli`) to integrate ratchet checks into `normalize rules run`
