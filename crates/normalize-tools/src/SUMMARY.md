# normalize-tools/src

Source for the normalize-tools crate.

Key modules: `adapters/` (per-tool implementations of the `Tool` trait), `test_runners/` (per-ecosystem `TestRunner` implementations), `tools.rs` (`Tool` trait, `ToolInfo`, `ToolResult`, `ToolCategory`, `ToolError`, `ToolInvocation` (structured command+args return type from `find_js_tool`/`find_python_tool`)), `diagnostic.rs` (`Diagnostic`, `DiagnosticSeverity`, `Fix`, `Location`), `registry.rs` (`ToolRegistry` with parallel `detect`/`run_detected`/`run_named`), `custom.rs` (TOML-driven `CustomTool` config), `sarif.rs` (`SarifReport` output format), `lib.rs` (top-level `default_registry` and `registry_with_custom`).
