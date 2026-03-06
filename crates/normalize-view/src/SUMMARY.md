# normalize-view/src

Source for the `normalize-view` crate.

`skeleton.rs` wraps `normalize_facts::Extractor` as `SkeletonExtractor`, defines `SymbolExt` (converts `Symbol` to `ViewNode`) and `ExtractResultExt` (converts `ExtractResult` to a file-rooted `ViewNode`). `tree.rs` implements `ViewNode`/`ViewNodeKind`, `generate_view_tree` (directory tree with optional skeleton expansion), `highlight_source` (tree-sitter highlights.scm query runner with injection support), `collect_highlight_spans`, and `format_view_node` (rendering with `FormatOptions` controlling indentation, line numbers, and pretty output). Highlight query compilation is cached in `OnceLock<RwLock<HashMap>>`.
