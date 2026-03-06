# normalize-facts-rules-builtins/src

Source for the builtin rule pack dylib.

- `lib.rs` — dylib entry point: `get_rule_pack()`, `info()`, `run()`, `run_rule()`; wires together all rule modules
- `circular_deps.rs` — Ascent Datalog rule detecting circular import dependencies between modules
