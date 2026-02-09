//! Report types for the view command's JSON output.
//!
//! Provides a tagged `ViewOutput` enum wrapping all 9 output modes of the view command,
//! enabling `--output-schema` support and consistent JSON serialization.

use crate::output::OutputFormatter;
use crate::tree::ViewNode;
use serde::Serialize;

/// Unified output type for the view command.
///
/// Each variant corresponds to one of the view command's output modes.
/// Internally tagged on `"mode"` so JSON always includes a discriminator.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "mode")]
pub enum ViewOutput {
    /// Directory tree view
    #[serde(rename = "directory")]
    Directory { node: ViewNode },

    /// File skeleton view
    #[serde(rename = "file")]
    File { node: ViewNode },

    /// Full symbol view (with source, imports)
    #[serde(rename = "symbol")]
    Symbol(ViewSymbolReport),

    /// Symbol found at a specific line number
    #[serde(rename = "symbol_at_line")]
    SymbolAtLine { node: ViewNode },

    /// Raw line range from a file
    #[serde(rename = "line_range")]
    LineRange(ViewLineRangeReport),

    /// Glob pattern matches within a file
    #[serde(rename = "glob_matches")]
    GlobMatches(ViewGlobReport),

    /// Git history for a symbol or file
    #[serde(rename = "history")]
    History(ViewHistoryReport),

    /// Symbols filtered by kind (class, function, method)
    #[serde(rename = "kind_filter")]
    KindFilter(ViewKindFilterReport),

    /// Multiple ambiguous matches (file + symbol)
    #[serde(rename = "multiple_matches")]
    MultipleMatches(ViewMultipleMatchesReport),

    /// Full file content (depth < 0 or depth > 2)
    #[serde(rename = "file_content")]
    FileContent(ViewFileContentReport),
}

/// Symbol view with source code and imports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewSymbolReport {
    pub path: String,
    pub file: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
}

/// Line range view.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewLineRangeReport {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub content: String,
}

/// Glob matches within a file.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewGlobReport {
    pub file: String,
    pub pattern: String,
    pub count: usize,
    pub matches: Vec<ViewGlobMatch>,
}

/// A single glob match entry.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewGlobMatch {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Git history for a symbol or file.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewHistoryReport {
    pub file: String,
    pub lines: String,
    pub commits: Vec<ViewHistoryCommit>,
}

/// A single commit in history output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewHistoryCommit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// Symbols filtered by kind.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewKindFilterReport {
    pub symbols: Vec<ViewKindFilterEntry>,
}

/// A single entry in kind-filtered output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewKindFilterEntry {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Multiple ambiguous matches.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewMultipleMatchesReport {
    pub file_matches: Vec<ViewFileMatch>,
    pub symbol_matches: Vec<ViewSymbolMatchEntry>,
}

/// A file/directory match in the multiple-matches response.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewFileMatch {
    pub path: String,
    #[serde(rename = "type")]
    pub match_type: String,
}

/// A symbol match in the multiple-matches response.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewSymbolMatchEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub match_type: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Full file content (when depth is outside 0..=2).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewFileContentReport {
    pub path: String,
    pub content: String,
}

impl OutputFormatter for ViewOutput {
    fn format_text(&self) -> String {
        // Text output is handled directly by each sub-function (syntax highlighting,
        // tree rendering, fisheye imports, etc.). This is only used as a fallback.
        match self {
            ViewOutput::Directory { .. }
            | ViewOutput::File { .. }
            | ViewOutput::Symbol(_)
            | ViewOutput::SymbolAtLine { .. }
            | ViewOutput::LineRange(_)
            | ViewOutput::GlobMatches(_)
            | ViewOutput::History(_)
            | ViewOutput::KindFilter(_)
            | ViewOutput::MultipleMatches(_)
            | ViewOutput::FileContent(_) => serde_json::to_string_pretty(self).unwrap_or_default(),
        }
    }
}
