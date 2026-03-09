# normalize-language-meta/src

Source for the `normalize-language-meta` crate.

`capabilities.rs` defines the `Capabilities` struct and its preset constructors (`all`, `none`, `data_format`, `markup`, `query`, `build_dsl`, `shell`). `data.rs` is the shared loader: parses `data/languages.toml` once via `OnceLock` into a `LanguageIndex` (`by_name` for exact-match capabilities, `by_id` for test-globs by lowercased name/alias/extension); also exposes `test_file_globs_for_language` as a public free function. `registry.rs` implements `capabilities_for` (checks `USER_CAPABILITIES` user override first, then delegates to `LanguageIndex`) and `register` (inserts into user override map).
