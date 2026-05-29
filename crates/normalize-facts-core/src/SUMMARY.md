# normalize-facts-core/src

Source files for the core facts data types.

- `symbol.rs` — `Symbol`, `FlatSymbol` (with `docstring: Option<String>` for index storage), `SymbolKind` (function/method/class/struct/enum/trait/interface/module/type/constant/variable/heading), `Visibility`
- `import.rs` — `Import`, `FlatImport` (with `is_reexport: bool` for `pub use` / `export...from` re-exports), `Export`
- `file.rs` — `IndexedFile` representing a file tracked in the index
- `type_ref.rs` — `TypeRef` and `TypeRefKind` for type reference facts
- `resolver.rs` — `InterfaceResolver` trait: cross-file interface method lookup used by `Language::post_process_symbols`; lives here (not in `normalize-facts`) so `normalize-languages` can reference it without a circular dependency
- `lib.rs` — re-exports all public types
