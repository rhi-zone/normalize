# normalize-syntax-rules/src

Source modules for the syntax rules crate.

- `lib.rs` — public API surface: re-exports `Rule`, `Severity`, `BuiltinRule`, and all key functions.
- `builtin/` — embedded builtin `.scm` rule files compiled in via `include_str!`.
- `loader.rs` — `load_all_rules()`, `parse_rule_content()`, `RulesConfig`, `RuleOverride`; merges rules from builtins, user global dir, and project dir.
- `runner.rs` — `run_rules()`, `apply_fixes()`, `Finding`, `DebugFlags`; executes rules against files using tree-sitter and handles per-line `normalize-syntax-allow:` suppression comments. `apply_fixes()` uses `if let Some` instead of `unwrap` for fix templates (audited as part of rust/unwrap-in-impl remediation).
- `query.rs` — `run_sexp_query()`, `run_astgrep_query()`, `is_sexp_pattern()`: dual query execution backends (tree-sitter native + ast-grep).
- `sources.rs` — `RuleSource` trait, `SourceRegistry`, `SourceContext`; built-in sources: `PathSource`, `EnvSource`, `GitSource`, `RustSource`, `GoSource`, `PythonSource`, `TypeScriptSource`.
- `main.rs` — binary entry point for the standalone `normalize-syntax-rules` CLI (gated behind `cli` feature)
- `service.rs` — `SyntaxRulesService` with `#[cli]` impl: `run` and `list` subcommands (gated behind `cli` feature)
