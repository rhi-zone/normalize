Source files for the normalize-rules crate.

- `lib.rs` — crate root; re-exports public API from runner and service modules.
- `runner.rs` — unified rule management: RuleType, RulesRunConfig, SarifTool, run_rules_report(), apply_native_rules_config(), collect_fact_diagnostics(), cmd_list/show/tags/enable_disable/add/update/remove, and diagnostic conversion helpers.
- `cmd_rules.rs` — syntax rule CLI command handler: cmd_rules(), run_syntax_rules(), build_rules_output(), SARIF output printer.
- `service.rs` — RulesService with #[cli] server-less registration; load_rules_config() parses rule config from .normalize/config.toml directly.
