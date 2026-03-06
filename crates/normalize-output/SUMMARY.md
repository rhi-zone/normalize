# normalize-output

Output formatting infrastructure shared across the normalize binary and `normalize-session-analysis`.

Key types: `OutputFormatter` trait (`format_text()` required, `format_pretty()` defaults to text), `PrettyConfig` (enabled/colors/highlight, mergeable via `Merge`), `ColorMode` (Auto/Always/Never), `DiagnosticsReport` and `Issue` (unified diagnostic types for all check/rule commands). Also exports `progress_bar`, `progress_bar_good`, `progress_bar_bad` helpers. `DiagnosticsReport` implements `OutputFormatter` and provides `format_sarif()` for SARIF 2.1.0 output. JSON/jq output is handled by the server-less proc macro, not here.
