# normalize-rules/src

Source files for the `normalize-rules` crate.

- `lib.rs` — crate root; re-exports public API from runner, loader, and service modules.
- `runner.rs` — unified rule management: `RuleKind`, `RulesRunConfig`, `run_rules_report()`, `apply_native_rules_config()`, `collect_fact_diagnostics()`, and helpers `enable_disable`/`show_rule`/`list_tags`/`add_rule`/`update_rules`/`remove_rule`. `RuleEntry` includes a `recommended` field. Lock file field is `content_hash` (uses `DefaultHasher`). `enable_disable` returns `Err` on inline-table `[rules]` rather than panicking. `global_allow` is applied to native-rule issues.
- `loader.rs` — dylib rule pack discovery (`search_paths`, `discover`), loading (`load_from_path`, `load_all`), and `format_diagnostic`. Inlined from the former `normalize-rules-loader` crate.
- `cmd_rules.rs` — syntax rule execution: `run_syntax_rules()` loads and runs tree-sitter syntax rules, returning raw `Finding` values.
- `service.rs` — `RulesService` with `#[cli]` server-less registration. `load_rules_config()` merges global + project config via `normalize_core::Merge`. Includes `RulesValidateReport`, `validate` (always returns `Ok`; callers check `report.valid`), `setup`, and `--fix` (loop capped at 100 iterations). `RuleShowReport` holds formatted output for `show`/`tags`/`enable`/`disable`.
- `setup.rs` — interactive rule setup wizard used by both `normalize rules setup` and `normalize init --setup`; groups violations by tag/category with qualitative impact labels.
