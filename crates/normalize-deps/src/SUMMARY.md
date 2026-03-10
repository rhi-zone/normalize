# normalize-deps/src

Single-file source for the `normalize-deps` crate.

`lib.rs` contains `DepsExtractor` with `extract(path, content) -> DepsResult`. `extract_with_trait` handles all languages except JS/TS/TSX: it tries `collect_imports_from_query` first (uses the language's `*.imports.scm` query — captures `@import`, `@import.path`, `@import.name`, `@import.alias`, `@import.glob`; aggregates multi-name imports by `@import` byte position), falling back to `collect_imports_with_trait` (Language trait walk) when no query exists, it fails to compile, or it produces no usable paths. JS/TS/TSX use `extract_js_ts_deps` for re-export support and CommonJS `require()` detection (`const x = require(...)`, `const { a, b } = require(...)`, bare `require(...)` side-effect imports). `extract_exports_from_tags` runs `tags.scm` and filters by `get_visibility()`. Embedded content (Vue, HTML) is handled by recursing into sub-trees with adjusted line numbers.
