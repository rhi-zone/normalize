# normalize-language-meta

Language metadata and capability classification, orthogonal to syntax extraction and package discovery.

Key types: `Capabilities` (booleans: `imports`, `callable_symbols`, `complexity`, `executable`). Key functions: `capabilities_for(language_name)` (looks up by `Language::name()`, defaults to `Capabilities::all()` for unknowns), `register(name, caps)` (user override), `test_file_globs_for_language(name)` (static glob patterns for test files, ~25 languages). Capability presets: `all()`, `none()`, `data_format()`, `markup()`, `query()`, `build_dsl()`, `shell()`. No dependencies; usable without tree-sitter.
