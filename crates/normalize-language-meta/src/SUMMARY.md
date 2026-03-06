# normalize-language-meta/src

Source for the `normalize-language-meta` crate.

`capabilities.rs` defines the `Capabilities` struct and its preset constructors. `registry.rs` implements `capabilities_for` (checks a `OnceLock<RwLock<HashMap>>` user override, then falls back to `builtin_capabilities` which classifies ~40 specific language names). `test_globs.rs` provides `test_file_globs_for_language` — a static match over language names returning `&'static [&'static str]` glob patterns for ~25 languages.
