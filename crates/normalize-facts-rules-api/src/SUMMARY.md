# normalize-facts-rules-api/src

Source files for the fact rule data types. The dylib/ABI-stable plugin interface has been removed; rules are now evaluated as interpreted `.dl` files via `normalize-facts-rules-interpret`.

- `relations.rs` — `Relations` struct holding the input fact tables passed to the Datalog engine: `SymbolFact`, `ImportFact`, `CallFact`, `VisibilityFact`, `AttributeFact`, `ParentFact`, `QualifierFact`, `SymbolRangeFact`, `ImplementsFact`, `IsImplFact`, `TypeMethodFact`; Phase 0 cross-file resolution facts: `ResolvedImportFact`, `ModuleFact`, `ExportFact`, `ReexportFact`, `SymbolUseFact`, `ResolvedReferenceFact`, `ResolvedCallFact`, `ModuleSearchPathFact`; plus `add_*` methods for each
- `diagnostic.rs` — `Diagnostic`, `DiagnosticLevel`, `Location` — the output type emitted by rules; uses plain Rust `String`/`Vec`/`Option` (no FFI types)
- `rule_pack.rs` — empty placeholder; previously held the ABI-stable vtable types for dylib loading
- `lib.rs` — re-exports all public types and re-exports `ascent`
