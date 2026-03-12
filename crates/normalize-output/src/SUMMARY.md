# normalize-output/src

Source for the `normalize-output` crate.

`lib.rs` defines `OutputFormatter`, `PrettyConfig`, `ColorMode`, and progress bar helpers. `diagnostics.rs` defines `Severity` (Hint/Info/Warning/Error, ordered), `Issue` (file/line/col/rule_id/message/severity/source/related/suggestion), `RelatedLocation`, `ToolError` (tool name + error message for failed SARIF tools), and `DiagnosticsReport` (issues + files_checked + sources_run + tool_errors). `DiagnosticsReport` provides `merge`, `sort`, `count_by_severity`, `format_text_limited(limit)`, `format_sarif()`, and full `OutputFormatter` with ANSI-colored `format_pretty()`. Tool errors are shown at the top of both text and pretty output.
