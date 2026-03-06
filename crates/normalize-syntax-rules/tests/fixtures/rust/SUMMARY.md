# fixtures/rust

Test fixtures for Rust syntax rules.

Each subdirectory corresponds to one builtin Rust rule and contains a `match.rs` file (expected to produce findings) and a `no_match.rs` file (expected to produce zero findings). Rules covered: `rust/chained-if-let`, `rust/dbg-macro`, `rust/expect-empty`, `rust/numeric-type-annotation`, `rust/println-debug`, `rust/static-mut`, `rust/todo-macro`, `rust/tuple-return`, `rust/unnecessary-let`, `rust/unnecessary-type-alias`, `rust/unwrap-in-impl`.
