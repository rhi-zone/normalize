# src/ast_grep/utils

Shared utilities for the ast-grep CLI embedding. Provides common CLI argument structs (`InputArgs`, `OutputArgs`, `ContextArgs`, `OverwriteArgs`), the `Worker` trait and `PathWorker`/`StdInWorker` implementations for parallel file processing, `ErrorContext` for structured error reporting, `DiffStyles` for diff rendering, `RuleOverwrite` for per-invocation rule overrides, and stub types (`FileTrace`, `RunTrace`, `ScanTrace`) for inspect features not included in the vendored subset.
