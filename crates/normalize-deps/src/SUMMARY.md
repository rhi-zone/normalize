# normalize-deps/src

Single-file source for the `normalize-deps` crate.

`lib.rs` exposes `extract_deps(path, content) -> DepsResult` (free function) and `DepsResult` (imports, exports, reexports, file_path). The private `DepsExtractor` struct namespaces the extraction methods: `extract_with_trait` handles all languages uniformly — tries `collect_imports_from_query` with `*.imports.scm` first (returns both imports and re-exports, honoring `@import.reexport` captures), falls back to `collect_imports_with_trait` if no `.scm` or no matches. `extract_exports_from_tags` runs `tags.scm` and filters by `get_visibility()`. Embedded content (Vue, HTML) is handled by recursing into sub-trees with adjusted line numbers. CommonJS `require()` (simple, destructured, aliased destructured, bare side-effect) and re-export patterns (`export * from`, `export * as ns from`, `export { name } from`) are handled entirely via the `.scm` query files.
