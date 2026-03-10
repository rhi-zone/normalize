# src/analyze

Re-export shim for backward compatibility within this crate. All types have moved to the
`normalize-metrics` crate (`crates/normalize-metrics/`).

`mod.rs` re-exports `complexity`, `function_length`, and `test_gaps` sub-modules from
`normalize_metrics`, plus the shared `FileReport<T>`, `FullStats`, `FunctionComplexity`,
and `FunctionLength` top-level types. Existing code in `commands/analyze/` and
`service/analyze.rs` continues to use `crate::analyze::*` paths unchanged.
