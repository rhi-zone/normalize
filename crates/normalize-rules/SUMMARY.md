Rule orchestration crate for normalize. Owns all rule management logic extracted from the main normalize crate.

Key modules:
- `src/runner.rs` — unified rule runner: list, run (syntax+fact+native+SARIF), show, tags, enable/disable, add/update/remove. Contains `RuleKind`, `RulesRunConfig`, `SarifTool`, `RulesListReport` + `RuleEntry` (structured list output with `OutputFormatter`), `build_list_report()`, `run_rules_report()`, `apply_native_rules_config()`, `collect_fact_diagnostics()`, and diagnostic conversion helpers (`finding_to_issue`, `abi_diagnostic_to_issue`). `enable_disable`, `show_rule`, and `list_tags` return `String` (callers print); `ListFilters` holds only `type_filter`, `tag`, `enabled`, `disabled` (no rendering flags).
- `src/loader.rs` — dylib rule pack discovery (`search_paths`, `discover`), loading (`load_from_path`, `load_all`), and `format_diagnostic`. Inlined from the former `normalize-rules-loader` crate (which has been removed).
- `src/cmd_rules.rs` — syntax rule CLI command handler (tree-sitter based, mirrors former `commands/analyze/rules_cmd.rs`).
- `src/service.rs` — `RulesService` with `#[cli(description = ...)]` proc-macro registration for server-less 0.4.0; feature-gated behind `cli`.

`RulesRunConfig` packages the rule-related config fields (syntax rules, fact rules, SARIF tools, rule-tags) without depending on `normalize`'s `NormalizeConfig`. `load_rules_config()` in `service.rs` parses these fields directly from `.normalize/config.toml`.
