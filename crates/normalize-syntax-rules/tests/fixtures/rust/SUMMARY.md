# fixtures/rust

Test fixtures for Rust syntax rules.

Most subdirectories correspond to one builtin Rust rule and contain a `match.rs` file (expected to produce findings) and a `no_match.rs` file (expected to produce zero findings). Rules covered by standard `match.*` / `no_match.*` fixtures: `rust/chained-if-let`, `rust/dbg-macro`, `rust/expect-empty`, `rust/numeric-type-annotation`, `rust/println-debug`, `rust/static-mut`, `rust/todo-macro`, `rust/tuple-return`, `rust/unnecessary-let`, `rust/unnecessary-type-alias`, `rust/unwrap-in-impl`. The `missing-module-doc/` subdirectory uses actual `lib.rs` named fixture files and is tested via the dedicated `test_rust_missing_module_doc` test function in `rule_fixtures.rs`.
