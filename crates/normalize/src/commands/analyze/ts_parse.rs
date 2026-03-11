//! Tree-sitter CST viewer — show the parse tree for a file.

use normalize_languages::{parsers::grammar_loader, support_for_grammar, support_for_path};
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single node in the parse tree (flat representation).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TreeNode {
    /// Node kind (e.g. `function_item`, `identifier`).
    pub kind: String,
    /// Field name within the parent node (e.g. `name`, `body`), if any.
    pub field: Option<String>,
    /// Start line (1-based).
    pub start_line: usize,
    /// Start column (0-based).
    pub start_col: usize,
    /// End line (1-based).
    pub end_line: usize,
    /// End column (0-based).
    pub end_col: usize,
    /// Source text, only set for named leaf nodes (no named children).
    pub text: Option<String>,
    /// Depth from root (or from the `--at` anchor node).
    pub depth: usize,
}

/// Report returned by `normalize analyze parse`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParseReport {
    /// File that was parsed.
    pub file: String,
    /// Detected or overridden language name.
    pub language: String,
    /// Total number of nodes collected.
    pub node_count: usize,
    /// Flat list of tree nodes in depth-first order.
    pub tree: Vec<TreeNode>,
}

impl OutputFormatter for ParseReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "File: {}  Language: {}  Nodes: {}\n",
            self.file, self.language, self.node_count
        ));
        for node in &self.tree {
            let indent = "  ".repeat(node.depth);
            let loc = format!(
                "[{}:{}-{}:{}]",
                node.start_line, node.start_col, node.end_line, node.end_col
            );
            if let Some(field) = &node.field {
                if let Some(text) = &node.text {
                    out.push_str(&format!(
                        "{}{}: {} {:?} {}\n",
                        indent, field, node.kind, text, loc
                    ));
                } else {
                    out.push_str(&format!("{}{}: {} {}\n", indent, field, node.kind, loc));
                }
            } else if let Some(text) = &node.text {
                out.push_str(&format!("{}{} {:?} {}\n", indent, node.kind, text, loc));
            } else {
                out.push_str(&format!("{}{} {}\n", indent, node.kind, loc));
            }
        }
        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;
        let mut out = String::new();
        out.push_str(&format!(
            "File: {}  Language: {}  Nodes: {}\n",
            Color::Cyan.bold().paint(&self.file),
            Color::Yellow.paint(&self.language),
            self.node_count
        ));
        for node in &self.tree {
            let indent = "  ".repeat(node.depth);
            let loc = format!(
                "[{}:{}-{}:{}]",
                node.start_line, node.start_col, node.end_line, node.end_col
            );
            let kind_str = Color::Cyan.paint(&node.kind).to_string();
            if let Some(field) = &node.field {
                let field_str = Color::Yellow.paint(field).to_string();
                if let Some(text) = &node.text {
                    let text_str = Color::Green.paint(format!("{:?}", text)).to_string();
                    out.push_str(&format!(
                        "{}{}: {} {} {}\n",
                        indent, field_str, kind_str, text_str, loc
                    ));
                } else {
                    out.push_str(&format!("{}{}: {} {}\n", indent, field_str, kind_str, loc));
                }
            } else if let Some(text) = &node.text {
                let text_str = Color::Green.paint(format!("{:?}", text)).to_string();
                out.push_str(&format!("{}{} {} {}\n", indent, kind_str, text_str, loc));
            } else {
                out.push_str(&format!("{}{} {}\n", indent, kind_str, loc));
            }
        }
        out
    }
}

/// Parse a file and return its CST as a `ParseReport`.
///
/// - `language_override`: if supplied, use this grammar name instead of detecting from extension.
/// - `at`: if supplied `(line, col)` (1-based line, 0-based col), anchor output to the smallest
///   node that contains that position.
/// - `depth`: maximum tree depth to include (relative to the anchor node).
pub fn parse_file(
    file: &Path,
    language_override: Option<&str>,
    at: Option<(usize, usize)>,
    depth: Option<usize>,
) -> Result<ParseReport, String> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| format!("could not read '{}': {}", file.display(), e))?;

    // Detect language.
    let lang_name: String = if let Some(lang) = language_override {
        lang.to_string()
    } else {
        support_for_path(file)
            .map(|l| l.name().to_string())
            .ok_or_else(|| {
                format!(
                    "unsupported file type: .{}",
                    file.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("<unknown>")
                )
            })?
    };

    // Load grammar.
    let loader = grammar_loader();
    let ts_lang = loader
        .get(&lang_name)
        .or_else(|| {
            // Also try by grammar name if language_override was a grammar name directly.
            support_for_grammar(&lang_name).and_then(|s| loader.get(s.name()))
        })
        .ok_or_else(|| format!("grammar not loaded for language '{lang_name}'"))?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_lang)
        .map_err(|e| format!("failed to set language: {e}"))?;

    let tree = parser
        .parse(&content, None)
        .ok_or_else(|| "tree-sitter parse failed".to_string())?;

    let root = tree.root_node();

    // If --at is given, find the smallest node containing that position.
    let anchor = if let Some((line, col)) = at {
        let point = tree_sitter::Point {
            row: line.saturating_sub(1), // convert to 0-based
            column: col,
        };
        find_node_at(root, point)
    } else {
        root
    };

    let max_depth = depth.unwrap_or(usize::MAX);
    let mut nodes: Vec<TreeNode> = Vec::new();
    collect_nodes(anchor, &content, None, 0, max_depth, &mut nodes);

    let node_count = nodes.len();
    Ok(ParseReport {
        file: file.display().to_string(),
        language: lang_name,
        node_count,
        tree: nodes,
    })
}

/// Walk the tree, collecting `TreeNode` entries depth-first.
fn collect_nodes(
    node: tree_sitter::Node<'_>,
    source: &str,
    field_name: Option<&str>,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<TreeNode>,
) {
    let start = node.start_position();
    let end = node.end_position();

    // Leaf text: only for named nodes with no named children.
    let text = if node.named_child_count() == 0 && node.is_named() {
        node.utf8_text(source.as_bytes()).ok().map(|s| {
            // Truncate very long leaf text to keep output readable.
            if s.len() > 120 {
                format!("{}…", &s[..120])
            } else {
                s.to_string()
            }
        })
    } else {
        None
    };

    out.push(TreeNode {
        kind: node.kind().to_string(),
        field: field_name.map(str::to_string),
        start_line: start.row + 1,
        start_col: start.column,
        end_line: end.row + 1,
        end_col: end.column,
        text,
        depth,
    });

    if depth < max_depth {
        let mut cursor = node.walk();
        for (i, child) in node.children(&mut cursor).enumerate() {
            // Obtain field name for this child by index.
            let fname = node.field_name_for_child(i as u32);
            collect_nodes(child, source, fname, depth + 1, max_depth, out);
        }
    }
}

/// Find the smallest (innermost) node that contains `point`.
fn find_node_at<'a>(
    root: tree_sitter::Node<'a>,
    point: tree_sitter::Point,
) -> tree_sitter::Node<'a> {
    let mut node = root;
    'outer: loop {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let s = child.start_position();
            let e = child.end_position();
            if (s.row < point.row || (s.row == point.row && s.column <= point.column))
                && (e.row > point.row || (e.row == point.row && e.column >= point.column))
            {
                node = child;
                continue 'outer;
            }
        }
        break;
    }
    node
}
