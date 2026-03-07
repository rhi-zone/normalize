# normalize-filter/src

Single-file source for the `normalize-filter` crate.

`lib.rs` implements `AliasConfig` (with `get_with_languages` for language-aware builtin resolution), `Filter::new` (builds `ignore::gitignore::Gitignore` matchers from patterns after alias expansion), `Filter::matches(path)`, `list_aliases` (for display), and `AliasStatus` (Builtin/Custom/Disabled/Overridden). The `@tests` alias calls `normalize_language_meta::test_file_globs_for_language` to produce language-specific patterns.

When the `cli` feature is enabled:
- `main.rs` — binary entry point for the standalone `normalize-filter` CLI
- `service.rs` — `FilterCliService` with `#[cli]` impl: `matches` and `aliases` subcommands
