//! Shared docstring extraction helpers.
//!
//! Free functions for extracting doc comments from AST nodes,
//! used by `Language::extract_docstring` implementations.

use tree_sitter::Node;

/// Extract a docstring from preceding comment nodes that use a given line prefix.
///
/// Walks backwards through siblings, collecting `comment` nodes whose text
/// starts with `prefix`. Stops at the first non-matching sibling.
///
/// # Example
///
/// ```ignore
/// // Lua: "---", R: "#'"
/// extract_preceding_prefix_comments(node, content, "---")
/// ```
pub(crate) fn extract_preceding_prefix_comments(
    node: &Node,
    content: &str,
    prefix: &str,
) -> Option<String> {
    let mut doc_lines: Vec<String> = Vec::new();
    let mut prev = node.prev_sibling();

    while let Some(sibling) = prev {
        if sibling.kind() == "comment" {
            let text = &content[sibling.byte_range()];
            if let Some(line) = text.strip_prefix(prefix) {
                let line = line.strip_prefix(' ').unwrap_or(line);
                doc_lines.push(line.to_string());
            } else {
                break;
            }
        } else {
            break;
        }
        prev = sibling.prev_sibling();
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    let joined = doc_lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}
