# normalize-facts-rules-api

Stable ABI for normalize rule plugins — the interface contract between the engine and Datalog rule packs loaded as dylibs at runtime.

Defines `RulePack` (the dylib export interface with `info`, `run`, `run_rule` function pointers), `RulePackInfo`, `RulePackRef`, `Relations` (the input fact tables: `SymbolFact`, `ImportFact`, `CallFact`), and `Diagnostic`/`DiagnosticLevel`/`Location` (rule output). Uses `abi_stable` for cross-version dylib safety and re-exports `ascent` for rule implementors. Rule packs implement this interface and are compiled to cdylib; the engine loads them via `RulePackRef`.
