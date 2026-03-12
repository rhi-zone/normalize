# normalize-facts/src

Source files for fact extraction and storage.

- `lib.rs` — public API re-exports
- `extract.rs` — `Extractor`, `ExtractOptions`, `ExtractResult`, `InterfaceResolver`, `OnDemandResolver`; drives per-file extraction using tree-sitter grammars and language trait hooks; uses `GrammarLoader::get_compiled_query()` for cached query compilation (tags, complexity); `collect_symbols_from_tags` supports arbitrary-depth container nesting via two-phase assembly (build symbols, then assemble tree bottom-up); nodes where `node_name()` returns None are skipped gracefully (not abort-all)
- `index.rs` — `FileIndex` (SQLite-backed store, schema v6), `CallGraphStats`, `ChangedFiles`, `SymbolMatch`; all index queries (`find_callers`, `find_callees`, `resolve_all_imports`, `resolve_all_calls`, etc.); `callee_resolved_file` in calls table enables cross-module disambiguation; `update_file()` for single-file incremental reindexing (used by LSP on save); `set_progress(true)` enables indicatif progress bars for `refresh()` and `refresh_call_graph()` (TTY-aware, hidden when stderr is not a terminal)
- `parsers.rs` — `grammar_loader`, `parser_for`, `parse_with_grammar`, `available_external_grammars`; manages tree-sitter grammar loading
- `symbols.rs` — `SymbolParser`: converts raw tree-sitter tag matches into `Symbol`/`FlatSymbol` records using `Language` trait hooks; `find_type_refs()` extracts type-to-type relationships (field_type, param_type, return_type, extends, implements, generic_bound, type_alias) for Rust, TypeScript/TSX, Python, Go, Java, C#, Kotlin, Swift, C++, and Ruby
- `main.rs` — binary entry point for the standalone `normalize-facts` CLI (gated behind `cli` feature)
- `service.rs` — `FactsCliService` with `#[cli]` impl: `rebuild`, `stats`, `files` subcommands (gated behind `cli` feature)
