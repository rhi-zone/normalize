# normalize-facts-rules-interpret

Interpreted Datalog rule evaluation for normalize code facts — runs `.dl` rule files directly without compilation using `ascent-interpreter`.

Bridges `Relations` (from `normalize-facts-rules-api`) to the ascent-interpreter engine so users can write `.dl` files that execute at runtime. Input relations (`symbol`, `import`, `call`, `visibility`, `attribute`, `parent`, `qualifier`, `symbol_range`, `implements`, `is_impl`, `type_method`) are pre-populated from the SQLite index. Output uses `warning`/`error`/`info` conventions in the Datalog. Ships a set of built-in `.dl` rule files in `src/builtin_dl/` covering architectural patterns (circular deps, god classes, dead API, etc.).
