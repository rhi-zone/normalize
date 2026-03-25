# normalize-facts-rules-api/src

Source files for the fact rule data types. The dylib/ABI-stable plugin interface has been removed; rules are now evaluated as interpreted `.dl` files via `normalize-facts-rules-interpret`.

- `relations.rs` — `Relations` struct holding the input fact tables (`SymbolFact`, `ImportFact`, `CallFact`, etc.) passed to the Datalog engine
- `diagnostic.rs` — `Diagnostic`, `DiagnosticLevel`, `Location` — the output type emitted by rules; uses plain Rust `String`/`Vec`/`Option` (no FFI types)
- `rule_pack.rs` — empty placeholder; previously held the ABI-stable vtable types for dylib loading
- `lib.rs` — re-exports all public types and re-exports `ascent`
