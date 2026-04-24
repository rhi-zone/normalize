# normalize-facts

Code fact extraction and storage — extracts symbols, imports, call graph data, and access-annotated calls from source files using tree-sitter and stores them in a SQLite database (via libsql).

Key exports: `FileIndex` (open/query the SQLite index), `Extractor` (walk a project and populate the index), `SymbolParser` (flatten tree-sitter parse results into `Symbol`/`Import` records), `ExtractOptions`/`ExtractResult`, `InterfaceResolver`/`OnDemandResolver` (import resolution strategies), `CallGraphStats`, `ChangedFiles`, `IndexedFile`, `SymbolMatch`. Re-exports all `normalize-facts-core` types for convenience. Depends on `normalize-languages` for grammar loading (via `grammar_loader`/`parser_for`/`parse_with_grammar`), `normalize-local-deps` for package discovery, and `indicatif` for progress bars. Test fixtures in `tests/fixtures/` cover symbol and import extraction across 30+ language samples.

Extraction is memoized via `normalize-ca-cache`: `refresh_call_graph` does a serial CA pre-pass (blake3 hash + CA get) before the rayon par-iter, extracting only uncached files. `reindex_files` (incremental path) checks and populates the CA cache per file. The CA cache is keyed by `(blake3(file_bytes), EXTRACTOR_VERSION, grammar_name)` and stored at `~/.config/normalize/ca-cache.sqlite`.

`CallEntry.access` is populated from the call graph index with read/write distinction when the language supports it. `ChangedFiles` tracks which files changed between index refreshes for incremental fact-rule evaluation via the daemon.

The `cli` feature adds a standalone `FactsCliService` (`src/service.rs`) with `rebuild`, `stats`, and `files` subcommands. Output types (`RebuildReport`, `StructureStatsReport`, `StructureFilesReport`) implement `OutputFormatter`.
