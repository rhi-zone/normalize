# normalize-facts-rules-interpret

Interpreted Datalog rule evaluation for normalize code facts — runs `.dl` rule files directly without compilation using `ascent-interpreter` (v0.1.1, with dynasm JIT).

Bridges `Relations` (from `normalize-facts-rules-api`) to the ascent-interpreter engine so users can write `.dl` files that execute at runtime. Input relations (`symbol`, `import`, `call`, `visibility`, `attribute`, `parent`, `qualifier`, `symbol_range`, `implements`, `is_impl`, `type_method`) are pre-populated from the SQLite index. Output uses `warning`/`error`/`info` conventions in the Datalog. Ships a set of built-in `.dl` rule files in `src/builtin_dl/` covering architectural patterns (circular deps, god classes, dead API, etc.).

`Severity`, `RuleOverride`, and `RulesConfig` are re-exported from `normalize-rules-config` (no local duplicates). `InterpretError` distinguishes `Parse` (bad Datalog syntax) from `Eval` (runtime evaluation failure) and `Io` (file read error).

JIT is NOT currently enabled: ascent-interpreter 0.1.1 has a bug where the JIT compares interned String values by intern ID (integer order) rather than lexicographic content, producing wrong results for rules using `if a < b` with String columns. The JIT infrastructure (`enable_jit`, `share_jit_compiler`, `set_jit_compiler`) is wired but commented out; re-enable when the upstream bug is fixed. The incremental path (`run_rules_source_incremental`) is available for daemon/LSP use where facts change between runs. The batch path (`run_rules_batch`) is structured for JIT sharing once the bug is resolved.
