# normalize-cfg/tests

Integration and snapshot tests for the CFG builder and Mermaid renderer.

- `rust_cfg.rs` — snapshot tests for the Rust CFG builder; one test per fixture in `fixtures/rust/`; Mermaid output is snapshot-tested with `insta`
- `python_cfg.rs` — snapshot tests for the Python CFG builder; skips gracefully if grammar not installed
- `go_cfg.rs` — snapshot tests for the Go CFG builder; skips gracefully if grammar not installed
- `typescript_cfg.rs` — snapshot tests for the TypeScript CFG builder; skips gracefully if grammar not installed
- `javascript_cfg.rs` — snapshot tests for the JavaScript CFG builder; skips gracefully if grammar not installed
- `java_cfg.rs` — snapshot tests for the Java CFG builder; skips gracefully if grammar not installed; validates labeled break/continue capture
- `lua_cfg.rs` — snapshot tests for the Lua CFG builder; skips gracefully if grammar incompatible
- `jinja2_cfg.rs` — snapshot tests for the Jinja2 CFG builder; processes whole template file (no function-level CFG)
- `coverage_matrix.rs` — classifies every registered language as HAS_CFG / NOT_APPLICABLE / DEFERRED; `cfg_has_cfg_languages_return_some` asserts all HAS_CFG grammars return `Some` from `get_cfg`; 76 languages now HAS_CFG, 3 DEFERRED (asm, x86asm, uiua)
- `fixtures/` — small source files for testing (one per language/control-flow pattern)
- `snapshots/` — insta snapshot files (auto-generated; update with `cargo insta review`)
