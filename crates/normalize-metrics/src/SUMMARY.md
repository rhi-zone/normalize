# normalize-metrics/src

Shared metric primitives for the ratchet and budget systems.

- `lib.rs` — `Metric` trait, `MetricFactory` type alias, re-exports
- `aggregate.rs` — `Aggregate` enum (Mean/Median/Max/Min/Sum/Count) and `aggregate()` function
- `filter.rs` — `filter_by_prefix()` for filtering `(key, value)` pairs by path prefix
