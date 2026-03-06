# normalize-facts/src

Source files for fact extraction and storage.

- `lib.rs` — public API re-exports
- `extract.rs` — `Extractor`, `ExtractOptions`, `ExtractResult`, `InterfaceResolver`, `OnDemandResolver`; drives per-file extraction using tree-sitter grammars and language trait hooks
- `index.rs` — `FileIndex` (SQLite-backed store), `CallGraphStats`, `ChangedFiles`, `SymbolMatch`; all index queries (`find_callers`, `find_callees`, `resolve_all_imports`, etc.)
- `parsers.rs` — `grammar_loader`, `parser_for`, `parse_with_grammar`, `available_external_grammars`; manages tree-sitter grammar loading
- `symbols.rs` — `SymbolParser`: converts raw tree-sitter tag matches into `Symbol`/`FlatSymbol` records using `Language` trait hooks
