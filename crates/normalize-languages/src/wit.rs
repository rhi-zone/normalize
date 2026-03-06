//! WebAssembly Interface Types (WIT) support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// WIT language support.
pub struct Wit;

impl Language for Wit {
    fn name(&self) -> &'static str {
        "WIT"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["wit"]
    }
    fn grammar_name(&self) -> &'static str {
        "wit"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "use_item" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // WIT: use interface.{func1, func2}
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("use {}", import.module)
        } else {
            format!("use {}.{{{}}}", import.module, names_to_use.join(", "))
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Items
            "export_item", "import_item",
            // Types
            "named_type", "extern_type", "func_type",
            // Enums/variants
            "enum_items", "enum_case", "variant_items", "variant_case",
            // Resources
            "resource_method",
            // Other
            "definitions", "body", "block_comment",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "interface_item",
            "type_item",
        ];
        validate_unused_kinds_audit(&Wit, documented_unused)
            .expect("WIT unused node kinds audit failed");
    }
}
