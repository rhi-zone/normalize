//! Report types for the view command's JSON output.
//!
//! Provides a unified `ViewReport` struct for all view output modes and
//! a `ViewListReport` newtype for lists. `OutputFormatter` impls render on demand:
//! `format_text()` = plain, `format_pretty()` = ANSI.

use crate::output::OutputFormatter;
use crate::tree::{
    DocstringDisplay, FormatOptions, ViewNode, ViewNodeKind, format_view_node, highlight_source,
};
use serde::Serialize;

/// Unified output type for the view command.
///
/// All view modes share this struct. Fields are populated based on the kind of
/// entity being viewed (directory, file, symbol, line range). Unpopulated fields
/// are omitted from JSON output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewReport {
    /// The resolved target path (file, dir, or symbol path)
    pub target: String,
    /// The tree node (always present — dir/file/symbol are all trees)
    pub node: ViewNode,
    /// Raw source text (for symbol views, line ranges, --full)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Import lines
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// Export lines
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<String>,
    /// Parent/ancestor signatures for context
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parent_signatures: Vec<String>,
    /// Line range if applicable (symbol, line target): (start, end) 1-indexed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_range: Option<(usize, usize)>,
    /// Grammar name for syntax highlighting (e.g. "rust", "python")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
    /// Warnings about unsupported features or missing capabilities
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Contents of SUMMARY.md for directory views (None if absent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// A list of `ViewReport` entries (returned by `normalize view list`).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ViewListReport(pub Vec<ViewReport>);

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

fn render_dir(report: &ViewReport, opts: &FormatOptions) -> String {
    let counts = count_dir_file_nodes(&report.node);
    let lines = format_view_node(&report.node, opts);
    let tree_text = format!(
        "{}\n\n{} directories, {} files",
        lines.join("\n"),
        counts.directories,
        counts.files
    );
    if let Some(summary) = &report.summary {
        format!("{}\n\n{}", summary.trim_end(), tree_text)
    } else {
        tree_text
    }
}

fn render_file_report(report: &ViewReport, opts: &FormatOptions) -> String {
    // --full mode: source present for a file node
    if let Some(src) = &report.source {
        let mut text = format!("# {}\n", report.target);
        if !report.warnings.is_empty() {
            text.push('\n');
            for w in &report.warnings {
                if opts.use_colors {
                    text.push_str(&format!("\x1b[33mwarning:\x1b[0m {}\n", w));
                } else {
                    text.push_str(&format!("warning: {}\n", w));
                }
            }
        }
        let rendered = if opts.use_colors {
            if let Some(g) = &report.grammar {
                highlight_source(src, g, true)
            } else {
                src.clone()
            }
        } else {
            src.clone()
        };
        text.push_str(&rendered);
        if !rendered.ends_with('\n') {
            text.push('\n');
        }
        return text;
    }

    // Skeleton mode: show imports/exports + symbol tree.
    // line_count: stored in the node's line_range end (we set line_range=(1, line_count) for files).
    let line_count = report.node.line_range.map(|(_, e)| e);
    let mut text = if let Some(lc) = line_count {
        format!("# {}\nLines: {}\n", report.target, lc)
    } else {
        format!("# {}\n", report.target)
    };

    if !report.warnings.is_empty() {
        text.push('\n');
        for w in &report.warnings {
            if opts.use_colors {
                text.push_str(&format!("\x1b[33mwarning:\x1b[0m {}\n", w));
            } else {
                text.push_str(&format!("warning: {}\n", w));
            }
        }
    }
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
    text
}

fn render_symbol_report(report: &ViewReport, use_colors: bool) -> String {
    let mut text = match report.line_range {
        Some((s, e)) => format!("# {} (L{}-{})\n", report.target, s, e),
        None => {
            if let Some((start, end)) = report.node.line_range {
                let kind_str = match &report.node.kind {
                    ViewNodeKind::Symbol(k) => k.as_str(),
                    _ => "",
                };
                if !kind_str.is_empty() {
                    format!("# {} ({}, L{}-{})\n", report.target, kind_str, start, end)
                } else {
                    format!("# {}\n", report.target)
                }
            } else {
                format!("# {}\n", report.target)
            }
        }
    };

    for sig in &report.parent_signatures {
        text.push_str(sig);
        text.push('\n');
    }
    if !report.parent_signatures.is_empty() {
        text.push('\n');
    }

    if !report.imports.is_empty() {
        for imp in &report.imports {
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
    } else {
        // Render tree node skeleton
        let opts = if use_colors {
            pretty_opts()
        } else {
            text_opts()
        };
        let lines = format_view_node(&report.node, &opts);
        for l in lines {
            text.push_str(&l);
            text.push('\n');
        }
    }
    text
}

fn render_line_range(report: &ViewReport, use_colors: bool) -> String {
    let header = match report.line_range {
        Some((s, e)) => format!("# {}:{}-{}\n\n", report.target, s, e),
        None => format!("# {}\n\n", report.target),
    };
    let content = report.source.as_deref().unwrap_or("");
    let rendered = if use_colors {
        if let Some(g) = &report.grammar {
            highlight_source(content, g, true)
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    };
    let mut text = header + &rendered;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

// --- OutputFormatter impl ---

impl OutputFormatter for ViewReport {
    fn format_text(&self) -> String {
        match &self.node.kind {
            ViewNodeKind::Directory => render_dir(self, &text_opts()),
            ViewNodeKind::File => render_file_report(self, &text_opts()),
            ViewNodeKind::Symbol(_) => {
                // Pure line range: line_range set, source present, no imports/parent context
                if self.line_range.is_some()
                    && self.source.is_some()
                    && self.parent_signatures.is_empty()
                    && self.imports.is_empty()
                {
                    render_line_range(self, false)
                } else {
                    render_symbol_report(self, false)
                }
            }
        }
    }

    fn format_pretty(&self) -> String {
        match &self.node.kind {
            ViewNodeKind::Directory => render_dir(self, &pretty_opts()),
            ViewNodeKind::File => render_file_report(self, &pretty_opts()),
            ViewNodeKind::Symbol(_) => {
                if self.line_range.is_some()
                    && self.source.is_some()
                    && self.parent_signatures.is_empty()
                    && self.imports.is_empty()
                {
                    render_line_range(self, true)
                } else {
                    render_symbol_report(self, true)
                }
            }
        }
    }
}

impl std::fmt::Display for ViewReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

impl OutputFormatter for ViewListReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for (i, r) in self.0.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&r.format_text());
        }
        out
    }

    fn format_pretty(&self) -> String {
        let mut out = String::new();
        for (i, r) in self.0.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&r.format_pretty());
        }
        out
    }
}

impl std::fmt::Display for ViewListReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

impl OutputFormatter for ViewHistoryReport {
    fn format_text(&self) -> String {
        let mut text = format!("History for {} (L{}):\n\n", self.file, self.lines);
        if self.commits.is_empty() {
            text.push_str("  No history found.");
        } else {
            for c in &self.commits {
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
}

impl std::fmt::Display for ViewHistoryReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Count of directories and files in a ViewNode tree.
pub struct DirFileCounts {
    pub directories: usize,
    pub files: usize,
}

/// Count directories and files in a ViewNode tree.
pub fn count_dir_file_nodes(node: &ViewNode) -> DirFileCounts {
    let mut dirs = 0usize;
    let mut files = 0usize;
    for child in &node.children {
        match child.kind {
            ViewNodeKind::Directory => {
                dirs += 1;
                let sub = count_dir_file_nodes(child);
                dirs += sub.directories;
                files += sub.files;
            }
            ViewNodeKind::File => files += 1,
            ViewNodeKind::Symbol(_) => {}
        }
    }
    DirFileCounts {
        directories: dirs,
        files,
    }
}
