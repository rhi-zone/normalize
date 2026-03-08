# normalize-deps

Module dependency extraction (imports, exports, re-exports) for all supported languages.

Key types: `DepsExtractor`, `DepsResult` (imports + exports + re-exports for a file), `ReExport`. Import extraction is query-first: `collect_imports_from_query` runs the language's `*.imports.scm` query (captures: `@import`, `@import.path`, `@import.name`, `@import.alias`, `@import.glob`); falls back to `Language::extract_imports` trait method if no query or the query fails to compile against the installed grammar. JavaScript/TypeScript/TSX use a separate path (`extract_js_ts_deps`) to capture re-exports. Exports are found via `tags.scm` `@definition.*` captures filtered by `get_visibility()`. Handles embedded content (Vue `<script>`, HTML `<script type="module">`).
