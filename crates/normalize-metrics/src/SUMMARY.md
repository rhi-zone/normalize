# normalize-metrics/src

Shared metric primitives for the ratchet and budget systems.

- `lib.rs` — `Metric` trait, `MetricFactory` type alias, re-exports
- `aggregate.rs` — `Aggregate` enum (Mean/Median/Max/Min/Sum/Count) and `compute_aggregate()` function; NaN/infinite values are filtered before aggregation
- `filter.rs` — `filter_by_prefix()` for filtering `(key, value)` pairs by path prefix; returns `MetricPoint` structs with `address` and `value` fields
