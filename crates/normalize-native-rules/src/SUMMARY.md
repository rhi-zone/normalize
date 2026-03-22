# src

Source for the `normalize-native-rules` crate. `lib.rs` re-exports the `build_*_report` functions and `NATIVE_RULES` descriptor array. `walk.rs` provides `gitignore_walk()` and `is_excluded_dir()` shared by all checks. Individual check modules: `check_refs.rs`, `stale_summary.rs`, `stale_docs.rs`, `check_examples.rs`, `ratchet.rs`, `budget.rs` — each produces a `DiagnosticsReport` for the rules engine.
