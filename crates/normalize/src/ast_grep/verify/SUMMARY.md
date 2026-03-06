# src/ast_grep/verify

Rule test harness for ast-grep, vendored from ast-grep 0.41.0. Implements the `normalize sg test` / `normalize ast-grep test` subcommand. Key types: `TestCase` (parses YAML rule test files), `CaseResult`/`CaseStatus` (pass/fail tracking), `TestHarness` / `FindFile` (test file discovery), `Reporter` trait with `DefaultReporter` and `InteractiveReporter`, `SnapshotCollection` (snapshot comparison for match output). Parallelises test execution across available CPU threads.
