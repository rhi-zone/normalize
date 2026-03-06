# normalize-deps

Module dependency extraction (imports, exports, re-exports) for all supported languages.

Key types: `DepsExtractor`, `DepsResult` (imports + exports + re-exports for a file), `ReExport`. The extractor uses the `Language` trait's `extract_imports` for most languages, with special-cased handling for JavaScript/TypeScript/TSX to capture re-exports (`export * from`, `export { x } from`). Exports are found via `tags.scm` `@definition.*` captures filtered by `get_visibility()`. Handles embedded content (Vue `<script>`, HTML `<script type="module">`).
