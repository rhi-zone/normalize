# src/service

Server-less `#[cli]` service layer — the primary CLI registration point. Each sub-module defines a service struct with methods annotated `#[cli(display_with = "...")]` that become subcommands. The `NormalizeService` root struct composes sub-services: `AnalyzeService`, `DaemonService`, `EditService`, `FactsService`, `GrammarService`, `GenerateService`, `PackageService`, `RulesService`, `ServeService`, `SessionsService`, `SyntaxService`, `ToolsService`. Helper methods (display_*, formatting) live in separate impl blocks above the `#[cli]` block. JSON/schema output is generated automatically by the proc macro; only `format_text()`/`format_pretty()` require manual implementation.

Service methods call underlying domain functions directly (not `cmd_*` wrappers — those were eliminated). `rules.rs` calls `cmd_list`, `cmd_run`, etc. from `commands/rules.rs` and wraps results with `exit_to_result()`.
