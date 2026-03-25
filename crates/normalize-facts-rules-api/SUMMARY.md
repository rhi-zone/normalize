# normalize-facts-rules-api

Data types for normalize fact rule evaluation — the shared interface between the Datalog engine and the rest of the system.

Defines `Relations` (the input fact tables: `SymbolFact`, `ImportFact`, `CallFact`, `VisibilityFact`, etc.) and `Diagnostic`/`DiagnosticLevel`/`Location` (rule output). All types use plain Rust `String`, `Vec`, and `Option` — no FFI types. Re-exports `ascent` for rule implementors. The former `abi_stable`/dylib interface (`RulePack`, `RulePackRef`) has been removed; rules now run as interpreted `.dl` files via `normalize-facts-rules-interpret`.
