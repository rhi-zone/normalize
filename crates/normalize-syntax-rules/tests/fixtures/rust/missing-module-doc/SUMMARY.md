# fixtures/rust/missing-module-doc

Fixtures for the `rust/missing-module-doc` rule.

Unlike most fixture directories, this one uses actual `lib.rs`-named files rather than `match.rs` / `no_match.rs` — because the rule's `files` inclusion filter restricts it to files named `lib.rs` and `mod.rs`. The `test_rust_missing_module_doc` function in `rule_fixtures.rs` tests these fixtures directly: `lib.rs` has no `//!` inner doc comment (expected to fire), and a temporary `lib.rs` with `//!` docs is created at test time (expected to pass).
