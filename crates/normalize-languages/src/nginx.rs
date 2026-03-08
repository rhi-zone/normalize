//! Nginx configuration file support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// Nginx language support.
pub struct Nginx;

impl Language for Nginx {
    fn name(&self) -> &'static str {
        "Nginx"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["nginx", "conf"]
    }
    fn grammar_name(&self) -> &'static str {
        "nginx"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "directive" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if let Some(rest) = text.strip_prefix("include ") {
            return vec![Import {
                module: rest.trim_end_matches(';').trim().to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard: text.contains('*'),
                is_relative: false,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Nginx: include path
        format!("include {}", import.module)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // For nginx blocks, the name is the directive name (server, location, etc.)
        if let Some(dir_node) = node.child_by_field_name("directive") {
            return Some(&content[dir_node.byte_range()]);
        }
        let mut cursor = node.walk();
        if let Some(first_child) = node.children(&mut cursor).next() {
            return Some(&content[first_child.byte_range()]);
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
            "lua_block", "lua_block_directive", "modifier",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "block",
        ];
        validate_unused_kinds_audit(&Nginx, documented_unused)
            .expect("Nginx unused node kinds audit failed");
    }
}
