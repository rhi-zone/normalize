//! Report types for the view command's JSON output.
//!
//! Provides a tagged `ViewOutput` enum wrapping all output modes of the view command,
//! enabling `--output-schema` support and consistent JSON serialization.
//! `OutputFormatter` impls render on demand: `format_text()` = plain, `format_pretty()` = ANSI.

use crate::output::OutputFormatter;
use crate::tree::{
    DocstringDisplay, FormatOptions, ViewNode, ViewNodeKind, format_view_node, highlight_source,
};
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
    File(ViewFileReport),

    /// Full symbol view (with source, imports)
    #[serde(rename = "symbol")]
    Symbol(ViewSymbolReport),

    /// Symbol found at a specific line number
    #[serde(rename = "symbol_at_line")]
    SymbolAtLine(ViewSymbolNodeReport),

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

/// File skeleton view with imports and symbol tree.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewFileReport {
    pub path: String,
    pub line_count: usize,
    /// Formatted import lines (e.g. "  from foo import bar")
    pub imports: Vec<String>,
    /// Formatted export lines
    pub exports: Vec<String>,
    pub node: ViewNode,
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
    /// Grammar name for syntax highlighting (e.g. "rust", "python")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
    /// Parent / ancestor signatures to show context
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parent_signatures: Vec<String>,
}

/// Symbol node view (skeleton, no raw source).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewSymbolNodeReport {
    pub node: ViewNode,
    /// Ancestor signatures for context
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parent_signatures: Vec<String>,
}

/// Line range view.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewLineRangeReport {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
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
    /// Source lines for this match
    pub source: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
}

// --- Rendering helpers ---

fn text_opts() -> FormatOptions {
    FormatOptions {
        minimal: true,
        use_colors: false,
        line_numbers: true,
        docstrings: DocstringDisplay::Summary,
        skip_root: false,
        ..Default::default()
    }
}

fn pretty_opts() -> FormatOptions {
    FormatOptions {
        minimal: false,
        use_colors: true,
        line_numbers: true,
        docstrings: DocstringDisplay::Summary,
        skip_root: false,
        ..Default::default()
    }
}

fn render_dir(node: &ViewNode, opts: &FormatOptions) -> String {
    let (dirs, files) = count_dir_file_nodes(node);
    let lines = format_view_node(node, opts);
    format!(
        "{}\n\n{} directories, {} files",
        lines.join("\n"),
        dirs,
        files
    )
}

fn render_file(report: &ViewFileReport, opts: &FormatOptions, use_colors: bool) -> String {
    let mut text = format!("# {}\nLines: {}\n", report.path, report.line_count);
    if !report.imports.is_empty() {
        text.push_str("\n## Imports\n");
        for imp in &report.imports {
            text.push_str(imp);
            text.push('\n');
        }
    }
    if !report.exports.is_empty() {
        text.push_str("\n## Exports\n");
        for exp in &report.exports {
            text.push_str(exp);
            text.push('\n');
        }
    }
    let child_opts = FormatOptions {
        skip_root: true,
        ..opts.clone()
    };
    let lines = format_view_node(&report.node, &child_opts);
    if !lines.is_empty() {
        text.push_str("\n## Symbols\n");
        for line in lines {
            text.push_str(&line);
            text.push('\n');
        }
    }
    let _ = use_colors; // handled via opts
    text
}

fn render_symbol(report: &ViewSymbolReport, use_colors: bool) -> String {
    let mut text = match (report.start_line, report.end_line) {
        (Some(s), Some(e)) => format!("# {} (L{}-{})\n", report.path, s, e),
        _ => format!("# {}\n", report.path),
    };
    for sig in &report.parent_signatures {
        text.push_str(sig);
        text.push('\n');
    }
    if !report.parent_signatures.is_empty() {
        text.push('\n');
    }
    if let Some(imports) = &report.imports
        && !imports.is_empty()
    {
        for imp in imports {
            text.push_str(imp);
            text.push('\n');
        }
        text.push('\n');
    }
    if let Some(source) = &report.source {
        let rendered = if use_colors {
            if let Some(g) = &report.grammar {
                highlight_source(source, g, true)
            } else {
                source.clone()
            }
        } else {
            source.clone()
        };
        text.push_str(&rendered);
        if !rendered.ends_with('\n') {
            text.push('\n');
        }
    }
    text
}

fn render_symbol_node(report: &ViewSymbolNodeReport, opts: &FormatOptions) -> String {
    let mut text = String::new();
    // Derive header from node metadata
    if let Some((start, end)) = report.node.line_range {
        let kind_str = match &report.node.kind {
            ViewNodeKind::Symbol(k) => k.as_str(),
            _ => "",
        };
        text.push_str(&format!(
            "# {} ({}, L{}-{})\n",
            report.node.name, kind_str, start, end
        ));
    }
    for sig in &report.parent_signatures {
        text.push_str(sig);
        text.push('\n');
    }
    if !report.parent_signatures.is_empty() {
        text.push('\n');
    }
    let lines = format_view_node(&report.node, opts);
    for l in lines {
        text.push_str(&l);
        text.push('\n');
    }
    text
}

// --- OutputFormatter impl ---

impl OutputFormatter for ViewOutput {
    fn format_text(&self) -> String {
        match self {
            ViewOutput::Directory { node } => render_dir(node, &text_opts()),
            ViewOutput::File(r) => render_file(r, &text_opts(), false),
            ViewOutput::Symbol(r) => render_symbol(r, false),
            ViewOutput::SymbolAtLine(r) => render_symbol_node(r, &text_opts()),
            ViewOutput::LineRange(r) => {
                let header = format!("# {}:{}-{}\n\n", r.file, r.start, r.end);
                let mut text = header + &r.content;
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text
            }
            ViewOutput::GlobMatches(r) => {
                let mut text = format!("# {}/{} ({} matches)\n\n", r.file, r.pattern, r.count);
                for m in &r.matches {
                    text.push_str(&format!(
                        "## {} ({}, L{}-{})\n",
                        m.path, m.kind, m.start_line, m.end_line
                    ));
                    text.push_str(&m.source);
                    if !m.source.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push('\n');
                }
                text
            }
            ViewOutput::History(r) => {
                let mut text = format!("History for {} (L{}):\n\n", r.file, r.lines);
                if r.commits.is_empty() {
                    text.push_str("  No history found.");
                } else {
                    for c in &r.commits {
                        text.push_str(&format!(
                            "  {} {} {} {}\n",
                            &c.hash[..8.min(c.hash.len())],
                            c.date,
                            c.author,
                            c.message
                        ));
                    }
                }
                text
            }
            ViewOutput::KindFilter(r) => {
                let mut text = String::new();
                for e in &r.symbols {
                    let parent_str = e
                        .parent
                        .as_ref()
                        .map(|p| format!(" (in {})", p))
                        .unwrap_or_default();
                    text.push_str(&format!(
                        "{}:{} {} {}{}\n",
                        e.file, e.line, e.kind, e.name, parent_str
                    ));
                }
                text.push_str(&format!("\n{} symbols found", r.symbols.len()));
                text
            }
            ViewOutput::MultipleMatches(r) => {
                let mut text = String::from("Multiple matches - be more specific:\n");
                for m in &r.file_matches {
                    text.push_str(&format!("  {} ({})\n", m.path, m.match_type));
                }
                for m in &r.symbol_matches {
                    let parent = m.parent.as_deref().unwrap_or("");
                    let sp = if parent.is_empty() {
                        m.name.clone()
                    } else {
                        format!("{}/{}", parent, m.name)
                    };
                    text.push_str(&format!("  {}/{} ({})\n", m.path, sp, m.kind));
                }
                text
            }
            ViewOutput::FileContent(r) => r.content.clone(),
        }
    }

    fn format_pretty(&self) -> String {
        match self {
            ViewOutput::Directory { node } => render_dir(node, &pretty_opts()),
            ViewOutput::File(r) => render_file(r, &pretty_opts(), true),
            ViewOutput::Symbol(r) => render_symbol(r, true),
            ViewOutput::SymbolAtLine(r) => render_symbol_node(r, &pretty_opts()),
            ViewOutput::LineRange(r) => {
                let header = format!("# {}:{}-{}\n\n", r.file, r.start, r.end);
                let highlighted = if let Some(g) = &r.grammar {
                    highlight_source(&r.content, g, true)
                } else {
                    r.content.clone()
                };
                let mut text = header + &highlighted;
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text
            }
            ViewOutput::FileContent(r) => {
                if let Some(g) = &r.grammar {
                    highlight_source(&r.content, g, true)
                } else {
                    r.content.clone()
                }
            }
            // No color difference for these variants
            other => other.format_text(),
        }
    }
}

impl std::fmt::Display for ViewOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Count directories and files in a ViewNode tree.
pub fn count_dir_file_nodes(node: &ViewNode) -> (usize, usize) {
    let mut dirs = 0usize;
    let mut files = 0usize;
    for child in &node.children {
        match child.kind {
            ViewNodeKind::Directory => {
                dirs += 1;
                let (sub_dirs, sub_files) = count_dir_file_nodes(child);
                dirs += sub_dirs;
                files += sub_files;
            }
            ViewNodeKind::File => files += 1,
            ViewNodeKind::Symbol(_) => {}
        }
    }
    (dirs, files)
}
