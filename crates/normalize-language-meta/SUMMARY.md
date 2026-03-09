# normalize-language-meta

Language metadata and capability classification, orthogonal to syntax extraction and package discovery.

Key types: `Capabilities` (booleans: `imports`, `callable_symbols`, `complexity`, `executable`). Key functions: `capabilities_for(language_name)` (looks up by `Language::name()`, defaults to `Capabilities::all()` for unknowns), `register(name, caps)` (user override), `test_file_globs_for_language(name)` (returns `Vec<String>` glob patterns for ~30 languages). Capability presets: `all()`, `none()`, `data_format()`, `markup()`, `query()`, `build_dsl()`, `shell()`. All language data lives in `data/languages.toml` (capabilities + test globs); parsed once at startup via `LanguageIndex` in `src/data.rs`. No hardcoded match arms. No dependencies beyond `serde` + `toml`.
