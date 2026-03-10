# normalize-metrics/src

Source code for the `normalize-metrics` crate.

- `lib.rs` — Crate root; re-exports `FunctionComplexity`, `FunctionLength`, `FileReport`, `FullStats`; defines shared `FileReport<T>` and `FullStats` structs
- `complexity.rs` — Cyclomatic complexity analysis: `ComplexityAnalyzer`, `FunctionComplexity`, `RiskLevel`, `ComplexityReport`; implements `OutputFormatter`
- `function_length.rs` — Function length analysis: `LengthAnalyzer`, `FunctionLength`, `LengthCategory`, `LengthReport`; implements `OutputFormatter`
- `test_gaps.rs` — Test gap analysis: `TestGapsReport`, `FunctionTestGap`, `DePriorityReason`, `compute_risk`, `check_de_priority`; implements `OutputFormatter`
