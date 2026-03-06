# normalize-syntax-rules/tests

Integration tests for the syntax rules crate.

`rule_fixtures.rs` contains a single `test_rule_fixtures` test that auto-discovers all fixture directories under `tests/fixtures/` and runs each rule against its `match.<ext>` file (must produce >= 1 findings) and `no_match.<ext>` file (must produce 0 findings). Rule IDs are derived from the fixture path relative to `tests/fixtures/` by joining path components with `/` (e.g. `fixtures/rust/static-mut/` → `rust/static-mut`). The `fixtures/` directory contains per-language subdirectories for Go, JS, Python, Ruby, Rust, and TypeScript, plus top-level directories for cross-language rules (`hardcoded-secret`, `no-fixme-comment`, `no-todo-comment`).
