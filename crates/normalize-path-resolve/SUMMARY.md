# normalize-path-resolve

Path resolution and fuzzy matching for normalize's unified path syntax (`file/Symbol`, `file::Symbol`, `@alias/path`).

Key types: `PathMatch` (path + kind + score), `UnifiedPath` (file_path + symbol_path + is_directory), `SigilExpansion`, `PathSource` trait (database-backed path lookup). Key functions: `resolve(query, root, path_source)` (fuzzy file lookup: exact → filename → suffix → nucleo fuzzy, top 10), `resolve_unified` (splits a query at the file/symbol boundary), `resolve_unified_all` (returns all matches for ambiguous queries), `expand_sigil` (handles `@alias` expansion), `all_files`. Supports separators: `/`, `::`, `:`, `#`.
