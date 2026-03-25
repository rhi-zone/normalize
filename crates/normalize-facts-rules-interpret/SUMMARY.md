# normalize-facts-rules-interpret

Interpreted Datalog rule evaluation for normalize code facts — runs `.dl` rule files directly without compilation using `ascent-interpreter` (v0.1.3, with dynasm JIT).

Bridges `Relations` (from `normalize-facts-rules-api`) to the ascent-interpreter engine so users can write `.dl` files that execute at runtime. Input relations (`symbol`, `import`, `call`, `visibility`, `attribute`, `parent`, `qualifier`, `symbol_range`, `implements`, `is_impl`, `type_method`) are pre-populated from the SQLite index. Output uses `warning`/`error`/`info` conventions in the Datalog. Ships a set of built-in `.dl` rule files in `src/builtin_dl/` covering architectural patterns (circular deps, god classes, dead API, etc.).

`Severity`, `RuleOverride`, and `RulesConfig` are re-exported from `normalize-rules-config` (no local duplicates). `InterpretError` distinguishes `Parse` (bad Datalog syntax) from `Eval` (runtime evaluation failure) and `Io` (file read error).

JIT is enabled (fixed in ascent-interpreter 0.1.3 — previously compared interned String values by intern ID rather than lexicographic content, producing wrong results for `if a < b` with String columns). `run_rules_source` enables JIT per engine; `run_rules_batch` enables JIT on the first rule then shares the compiler across subsequent rules via `share_jit_compiler`/`set_jit_compiler`. The incremental path (`run_rule_with_cache`, `run_rule_incremental`) is used by the daemon/LSP path via `normalize-rules::collect_fact_diagnostics_incremental`.
