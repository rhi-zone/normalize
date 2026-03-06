//! Meson build system support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Meson language support.
pub struct Meson;

impl Language for Meson {
    fn name(&self) -> &'static str {
        "Meson"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["meson.build", "meson_options.txt"]
    }
    fn grammar_name(&self) -> &'static str {
        "meson"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "normal_command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if text.starts_with("subproject(") || text.starts_with("dependency(") {
            return vec![Import {
                module: text.to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: false,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Meson: subdir('path')
        format!("subdir('{}')", import.module)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        if let Some(left_node) = node.child_by_field_name("left") {
            return Some(&content[left_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
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
            // Control flow commands
            "else_command", "elseif_command",
            // Expression-related
            "formatunit", "identifier", "operatorunit", "ternaryoperator",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "expression_statement",
            "foreach_command",
            "if_condition",
            "if_command",
        ];
        validate_unused_kinds_audit(&Meson, documented_unused)
            .expect("Meson unused node kinds audit failed");
    }
}
