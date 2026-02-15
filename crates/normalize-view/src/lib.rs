//! View tree, skeleton extraction, and syntax highlighting.

pub mod skeleton;
pub mod tree;

pub use skeleton::{
    ExtractResultExt, SkeletonExtractor, SkeletonResult, SkeletonSymbol, SymbolExt,
};
pub use tree::{
    DEFAULT_BOILERPLATE_DIRS, DocstringDisplay, FormatOptions, HighlightKind, HighlightSpan,
    TreeOptions, ViewNode, ViewNodeKind, collect_highlight_spans, docstring_summary,
    format_view_node, generate_view_tree, highlight_source,
};
