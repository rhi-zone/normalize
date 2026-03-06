# normalize-facts-rules-api/src

Source files for the stable rule plugin ABI.

- `rule_pack.rs` — `RulePack` vtable struct, `RulePackInfo`, `RulePackRef` (ABI-stable wrapper for dynamic loading)
- `relations.rs` — `Relations` struct holding the input fact tables (`SymbolFact`, `ImportFact`, `CallFact`) passed to rule packs
- `diagnostic.rs` — `Diagnostic`, `DiagnosticLevel`, `Location` — the output type emitted by rules
- `lib.rs` — re-exports all public types and re-exports `ascent` and `abi_stable` types needed by plugin implementors
