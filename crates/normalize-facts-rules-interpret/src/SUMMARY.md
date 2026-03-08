# normalize-facts-rules-interpret/src

Source for the interpreted Datalog rule engine.

- `lib.rs` — main engine: loads `.dl` files, populates relations from the index, runs ascent-interpreter, converts output to `Diagnostic` values; also handles rule config (TOML frontmatter in `.dl` files) and rule discovery. Output relations: `warning(rule_id, message)`, `error(rule_id, message)` (locationless), and `diagnostic(severity, rule_id, file, line, message)` (file-located; severity = "warning"/"error"/"info"/"hint"; line=0 when no line info). Note: unsuffixed integer literals (e.g. `0`) evaluate as `i32` in the ascent-interpreter; use `0i32` or handle both in extraction until the interpreter adds type-aware coercion.
- `tests.rs` — unit tests for the rule engine, extracted from lib.rs to keep test code in a dedicated file
- `builtin_dl/` — bundled `.dl` rule files for built-in architectural checks
