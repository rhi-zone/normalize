# normalize-facts-rules-interpret

Interpreted Datalog rule evaluation for normalize code facts — runs `.dl` rule files directly without compilation using `ascent-interpreter` (v0.1.5).

Bridges `Relations` (from `normalize-facts-rules-api`) to the ascent-interpreter engine so users can write `.dl` files that execute at runtime. Input relations (`symbol`, `import`, `call`, `visibility`, `attribute`, `parent`, `qualifier`, `symbol_range`, `implements`, `is_impl`, `type_method`) are pre-populated from the SQLite index. Output uses `warning`/`error`/`info` conventions in the Datalog. Ships a set of built-in `.dl` rule files in `src/builtin_dl/` covering architectural patterns (circular deps, god classes, dead API, etc.).

`Severity`, `RuleOverride`, and `RulesConfig` are re-exported from `normalize-rules-config` (no local duplicates). `InterpretError` distinguishes `Parse` (bad Datalog syntax) from `Eval` (runtime evaluation failure) and `Io` (file read error).

JIT support: ascent-interpreter is depended on with `default-features = false` to disable the `jit-asm` feature (which pulls in `dynasmrt` with x86-only assembly). A local `jit` feature re-enables JIT when desired. The `SharedJitCompiler` import and all JIT call sites are guarded with `#[cfg(all(target_arch = "x86_64", feature = "jit"))]` so aarch64, Windows, and non-JIT builds compile cleanly. `run_rules_source` enables JIT per engine on x86_64+jit; `run_rules_batch` enables JIT on the first rule then shares the compiler via `share_jit_compiler`/`set_jit_compiler`. The incremental path (`run_rule_with_cache`, `run_rule_incremental`) is used by the daemon/LSP path via `normalize-rules::collect_fact_diagnostics_incremental`.
