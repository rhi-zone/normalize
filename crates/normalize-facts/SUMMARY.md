# normalize-facts

Code fact extraction and storage — extracts symbols, imports, and call graph data from source files using tree-sitter and stores them in a SQLite database (via libsql).

Key exports: `FileIndex` (open/query the SQLite index), `Extractor` (walk a project and populate the index), `SymbolParser` (flatten tree-sitter parse results into `Symbol`/`Import` records), `ExtractOptions`/`ExtractResult`, `InterfaceResolver`/`OnDemandResolver` (import resolution strategies). Re-exports all `normalize-facts-core` types for convenience. Depends on `normalize-languages` for grammar loading, `normalize-local-deps` for package discovery, and `indicatif` for progress bars. Test fixtures in `tests/fixtures/` cover symbol and import extraction across 30+ language samples.

The `cli` feature adds a standalone `FactsCliService` (`src/service.rs`) with `rebuild`, `stats`, and `files` subcommands. Output types (`RebuildResult`, `StructureStatsReport`, `StructureFilesReport`) implement `OutputFormatter` and `Display`.
