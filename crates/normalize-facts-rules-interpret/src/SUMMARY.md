# normalize-facts-rules-interpret/src

Source for the interpreted Datalog rule engine.

- `lib.rs` — main engine: loads `.dl` files, populates relations from the index, runs ascent-interpreter, converts output to `Diagnostic` values; also handles rule config (TOML frontmatter in `.dl` files) and rule discovery. Single output relation: `diagnostic(severity, rule_id, file, line, message)` (severity = "warning"/"error"/"info"/"hint"; file = "" for no location; line = 0 when no line info). Exports `run_rules_source` (single rule), `run_rules_source_incremental` (delta-only re-eval for daemon path), and `run_rules_batch` (batch of rules, structured for JIT sharing once upstream bug is fixed). JIT is disabled pending ascent-interpreter bug fix (String comparison uses intern ID not content). Unsuffixed integer literals (e.g. `0`) are coerced to the declared column type by the interpreter.
- `tests.rs` — unit tests for the rule engine, extracted from lib.rs to keep test code in a dedicated file
- `builtin_dl/` — bundled `.dl` rule files for built-in architectural checks
