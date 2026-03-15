Source files for the normalize-rules crate.

- `lib.rs` — crate root; re-exports public API from runner, loader, and service modules.
- `runner.rs` — unified rule management: RuleType, RulesRunConfig, SarifTool, run_rules_report(), apply_native_rules_config(), collect_fact_diagnostics(), enable_disable/show_rule/list_tags/add_rule/update_rules/remove_rule, and diagnostic conversion helpers. RuleEntry includes `recommended` field.
- `loader.rs` — dylib rule pack discovery (search_paths, discover), loading (load_from_path, load_all), and format_diagnostic. Inlined from the former normalize-rules-loader crate.
- `cmd_rules.rs` — syntax rule execution: run_syntax_rules() loads and runs tree-sitter syntax rules, returning raw Finding values.
- `service.rs` — RulesService with #[cli] server-less registration; load_rules_config() parses rule config from .normalize/config.toml directly. Includes RulesValidateReport, `validate`, and `setup` subcommands.
- `setup.rs` — interactive rule setup wizard: runs all rules, groups violations by tag/category (correctness → security → style → …), shows qualitative impact labels (quick fix / moderate / major cleanup), and offers per-rule and batch enable/disable. Also used by `normalize init --setup`.
