# normalize-facts-core

Core data types for the normalize facts system — the shared vocabulary for code facts across the ecosystem.

Defines `Symbol` (with `SymbolKind` and `Visibility`), `Import`, `Export`, `FlatSymbol`, `FlatImport`, `IndexedFile`, and `TypeRef`. These types are used by `normalize-facts` for extraction and storage, by `normalize-facts-rules-api` for Datalog analysis, and by `normalize-languages` for language-specific extraction. No logic lives here — only serializable data types.
