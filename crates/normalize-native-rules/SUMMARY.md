# normalize-native-rules

Native rule check implementations for normalize. Provides four built-in checks that run without tree-sitter parsing: `stale-summary` (SUMMARY.md freshness via git history), `check-refs` (broken symbol references in markdown docs), `stale-docs` (documentation with stale `covers:` annotations), and `check-examples` (missing example references in docs). All checks produce `DiagnosticsReport` (from `normalize-output`) and are called by the native engine in `normalize rules run` and `normalize analyze check`.
