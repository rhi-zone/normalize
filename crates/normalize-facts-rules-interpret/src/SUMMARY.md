# normalize-facts-rules-interpret/src

Source for the interpreted Datalog rule engine.

- `lib.rs` — main engine: loads `.dl` files, populates relations from the index, runs ascent-interpreter, converts output to `Diagnostic` values; also handles rule config (TOML frontmatter in `.dl` files) and rule discovery
- `tests.rs` — unit tests for the rule engine, extracted from lib.rs to keep test code in a dedicated file
- `builtin_dl/` — bundled `.dl` rule files for built-in architectural checks
