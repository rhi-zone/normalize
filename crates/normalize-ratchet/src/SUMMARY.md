# src

Source for the `normalize-ratchet` crate. `lib.rs` exports the `Metric` trait. Domain logic modules: `baseline.rs` (file I/O), `check.rs` (regression detection), `update.rs` (baseline update, ratchet and force modes), `complexity.rs` (complexity metric with injected measurement fn). CLI module: `service.rs` (behind `cli` feature; `RatchetService` with `check`, `update`, `show` subcommands and `OutputFormatter` impls for all three report structs).
