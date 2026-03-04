//! KDL (KDocument Language) support.

use crate::{ContainerBody, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// KDL language support.
pub struct Kdl;

impl Language for Kdl {
    fn name(&self) -> &'static str {
        "KDL"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["kdl"]
    }
    fn grammar_name(&self) -> &'static str {
        "kdl"
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
        if node.kind() != "node" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
            signature: first_line.trim().to_string(),
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // KDL: node → node_no_terminator → children field (node_children: "{ ... }")
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.kind() == "node_no_terminator" {
                return child.child_by_field_name("children");
            }
        }
        None
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // node_children is "{ ... }" — use brace body analysis
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // First child is typically the identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "string" {
                return Some(&content[child.byte_range()]);
            }
        }
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
            "annotation_type",       // Type annotations
            "identifier_string",     // Identifiers as strings
            "multi_line_string_body", // Multi-line string content
            "type",                  // Type annotations
        ];
        validate_unused_kinds_audit(&Kdl, documented_unused)
            .expect("KDL unused node kinds audit failed");
    }
}
