# normalize-deps/src

Single-file source for the `normalize-deps` crate.

`lib.rs` exposes `extract_deps(path, content) -> DepsResult` (free function) and `DepsResult` (imports, exports, reexports, file_path). The private `DepsExtractor` struct namespaces the extraction methods: `extract_with_trait` handles all languages except JS/TS/TSX (tries `collect_imports_from_query` with `*.imports.scm` first, falls back to `collect_imports_with_trait`); JS/TS/TSX use `extract_js_ts_deps` for re-export support and CommonJS `require()` detection. `extract_exports_from_tags` runs `tags.scm` and filters by `get_visibility()`. Embedded content (Vue, HTML) is handled by recursing into sub-trees with adjusted line numbers.
