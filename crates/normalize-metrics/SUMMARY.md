# normalize-metrics

Code quality metrics crate extracted from the main `normalize` crate.

Provides tree-sitter-backed analyzers that work on source text and produce structured reports
implementing `OutputFormatter`. Used by `normalize analyze complexity`, `normalize analyze length`,
and `normalize analyze test-gaps` commands.

## Modules

- `src/complexity.rs` — McCabe cyclomatic complexity: `ComplexityAnalyzer`, `FunctionComplexity`, `RiskLevel`
- `src/function_length.rs` — Function line-count analysis: `LengthAnalyzer`, `FunctionLength`, `LengthCategory`
- `src/test_gaps.rs` — Untested public function detection: `TestGapsReport`, risk scoring, de-prioritization logic
- `src/lib.rs` — Shared `FileReport<T>` and `FullStats` structs

## Dependencies

- `normalize-analyze` — `Entity` trait for ranked-list infrastructure
- `normalize-facts` — `compute_complexity` (complexity query runner)
- `normalize-languages` — `Language` trait, `support_for_path`, `GrammarLoader`, `parsers`
- `normalize-output` — `OutputFormatter` trait
