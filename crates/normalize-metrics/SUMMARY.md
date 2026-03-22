# normalize-metrics

Shared metric primitives used by `normalize-ratchet` and `normalize-budget`.

Provides:
- `Metric` trait: snapshot metrics returning `Vec<(String, f64)>` via `measure_all`
- `MetricFactory` type alias: `fn(root: &Path) -> Vec<Box<dyn Metric>>`
- `Aggregate` enum and `aggregate()` function for reducing value lists
- `filter_by_prefix()` for filtering `(key, value)` pairs by path prefix
