# src/analyze

Core analysis passes used internally by multiple commands. Contains three modules: `complexity.rs` (cyclomatic/cognitive complexity per function, `FunctionComplexity`), `function_length.rs` (line count per function, `FunctionLength`), and `test_gaps.rs` (detection of untested functions). Results are wrapped in the shared `FileReport<T>` / `FullStats` types. This module is the computation layer; formatting and CLI wiring live in `commands/analyze/` and `service/analyze.rs`.
