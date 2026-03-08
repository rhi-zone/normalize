# normalize-languages/src/queries

Tree-sitter `.scm` query files for symbol extraction, call graph, complexity, imports, and type analysis.

315 files covering 77 languages. Query types per language: `<lang>.tags.scm` (symbol definitions — functions, classes, types), `<lang>.calls.scm` (function call sites), `<lang>.complexity.scm` (cyclomatic complexity nodes), `<lang>.imports.scm` (import/require statements), `<lang>.types.scm` (type definitions). Not every language has all five query types — coverage varies by what the grammar models. These files drive the index extraction in `normalize-facts`; node classification stays in `.scm` while name/field extraction from matched nodes stays in Rust. All `.tags.scm` and `.types.scm` files are registered in `grammar_loader.rs` via `bundled_tags_query` / `bundled_types_query`.
