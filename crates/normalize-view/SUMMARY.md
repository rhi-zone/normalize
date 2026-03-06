# normalize-view

View tree construction, skeleton extraction, and syntax highlighting for the `normalize view` command.

This crate was previously the home of view logic; its implementation has moved into the main `normalize` binary but this crate is retained as the published library facade. Key exports: `SkeletonExtractor`, `SkeletonResult` (alias for `ExtractResult`), `SkeletonSymbol` (alias for `Symbol`), `SymbolExt` and `ExtractResultExt` traits (convert to `ViewNode`), and from `tree`: `ViewNode`, `ViewNodeKind`, `generate_view_tree`, `highlight_source`, `collect_highlight_spans`, `format_view_node`, `FormatOptions`, `TreeOptions`, `DocstringDisplay`, `HighlightKind`, `HighlightSpan`.
