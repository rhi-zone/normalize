//! Diff/patch file support.

use crate::{Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Diff language support.
pub struct Diff;

impl Language for Diff {
    fn name(&self) -> &'static str {
        "Diff"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["diff", "patch"]
    }
    fn grammar_name(&self) -> &'static str {
        "diff"
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "file_change" {
            return None;
        }

        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        // Extract filename from diff header
        let name = first_line
            .split_whitespace()
            .find(|s| s.contains('/') || s.contains('.'))
            .map(|s| s.trim_start_matches("a/").trim_start_matches("b/"))
            .unwrap_or("unknown");

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
            signature: first_line.to_string(),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block", // Hunk block
        ];
        validate_unused_kinds_audit(&Diff, documented_unused)
            .expect("Diff unused node kinds audit failed");
    }
}
