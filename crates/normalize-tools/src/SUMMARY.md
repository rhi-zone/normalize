# normalize-tools/src

Source for the normalize-tools crate.

Key modules: `adapters/` (per-tool implementations of the `Tool` trait), `test_runners/` (per-ecosystem `TestRunner` implementations), `tools.rs` (`Tool` trait, `ToolInfo`, `ToolResult`, `ToolCategory`, `ToolError`, `ToolInvocation`; `find_js_tool`/`find_python_tool` now take `root: &Path` to resolve local installs relative to the project root), `diagnostic.rs` (`Diagnostic`, `DiagnosticSeverity` (re-exported from `normalize-output::Severity`), `Fix`, `Location`), `registry.rs` (`ToolRegistry` with parallel `detect`/`run_detected`/`run_named`), `custom.rs` (TOML-driven `CustomTool` config; strings are interned via global cache to avoid repeated `Box::leak`), `sarif.rs` (`SarifReport` output format), `lib.rs` (top-level `default_registry` and `registry_with_custom`).
