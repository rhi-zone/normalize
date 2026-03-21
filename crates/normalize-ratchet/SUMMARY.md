# normalize-ratchet

Metric regression-tracking ("ratchet") crate. Stores a baseline of metric values in `.normalize/ratchet.json` (checked into git) and flags when current values regress past the baseline.

## Structure

- `src/lib.rs` — crate root; exports the `Metric` trait
- `src/baseline.rs` — baseline file I/O (`.normalize/ratchet.json` load/save)
- `src/check.rs` — regression detection logic (`check_against_baseline`)
- `src/update.rs` — baseline update logic (`compute_update`, ratchet and force modes)
- `src/complexity.rs` — complexity metric; uses an injected `MeasureFn` to avoid circular deps
- `src/service.rs` — CLI service behind `cli` feature: `check`, `update`, `show` subcommands

## Features

- `cli` (optional): enables `RatchetService` with `#[cli]` proc-macro integration and `OutputFormatter` impls

## Key types

- `Metric` trait: `name()`, `measure(&Path)`, `is_regression(baseline, current)`
- `Baseline`: serde-able baseline file struct
- `MetricFactory`: `fn() -> Vec<Box<dyn Metric>>` for injecting metrics at construction time
- `RatchetCheckReport`, `RatchetUpdateReport`, `RatchetShowReport`: CLI report structs
