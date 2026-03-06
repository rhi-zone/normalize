# normalize-facts-rules-builtins

The default rule pack that ships with normalize, compiled as a cdylib and loaded at runtime via the `normalize-facts-rules-api` ABI.

Currently implements circular dependency detection (`circular-deps` rule via `circular_deps.rs` using Ascent Datalog). Exports `get_rule_pack()` as the dylib entry point returning a `RulePackRef`. Built as both `cdylib` (for dynamic loading) and `rlib` (for testing). Additional rules are added as modules and wired into `info()` and `run()`/`run_rule()`.
