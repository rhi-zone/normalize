Source files for the normalize-rules crate.

- `lib.rs` — crate root; re-exports public API from runner and service modules.
- `runner.rs` — unified rule management: RuleType, RulesRunConfig, SarifTool, run_rules_report(), apply_native_rules_config(), collect_fact_diagnostics(), enable_disable/show_rule/list_tags/add_rule/update_rules/remove_rule, and diagnostic conversion helpers.
- `cmd_rules.rs` — syntax rule execution: run_syntax_rules() loads and runs tree-sitter syntax rules, returning raw Finding values.
- `service.rs` — RulesService with #[cli] server-less registration; load_rules_config() parses rule config from .normalize/config.toml directly. Includes RulesValidateReport and the `validate` subcommand.
