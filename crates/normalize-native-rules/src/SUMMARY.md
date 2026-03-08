# src

Source for the `normalize-native-rules` crate. `lib.rs` re-exports the four `build_*_report` functions. `walk.rs` provides `gitignore_walk()` and `is_excluded_dir()` shared by all checks. Individual check modules: `check_refs.rs`, `stale_summary.rs`, `stale_docs.rs`, `check_examples.rs` — each defines a report struct with `OutputFormatter` and `From<Report> for DiagnosticsReport`.
