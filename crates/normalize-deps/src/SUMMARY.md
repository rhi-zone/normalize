# normalize-deps/src

Single-file source for the `normalize-deps` crate.

`lib.rs` contains `DepsExtractor` with `extract(path, content) -> DepsResult`. Trait-based extraction (`extract_with_trait`) handles all languages except JS/TS/TSX, which use `extract_js_ts_deps` for re-export support. `extract_exports_from_tags` runs the language's `tags.scm` query and filters by `get_visibility()`. Embedded content (Vue, HTML) is handled by recursing into sub-trees with adjusted line numbers.
