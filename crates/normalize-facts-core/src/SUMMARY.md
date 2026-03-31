# normalize-facts-core/src

Source files for the core facts data types.

- `symbol.rs` — `Symbol`, `FlatSymbol` (with `docstring: Option<String>` for index storage), `SymbolKind` (function/method/class/struct/enum/trait/interface/module/type/constant/variable/heading), `Visibility`
- `import.rs` — `Import`, `FlatImport`, `Export`
- `file.rs` — `IndexedFile` representing a file tracked in the index
- `type_ref.rs` — `TypeRef` and `TypeRefKind` for type reference facts
- `lib.rs` — re-exports all public types
