//! Apache Thrift IDL support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// Thrift language support.
pub struct Thrift;

impl Language for Thrift {
    fn name(&self) -> &'static str {
        "Thrift"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["thrift"]
    }
    fn grammar_name(&self) -> &'static str {
        "thrift"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "include_statement" {
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Thrift: include "file.thrift"
        format!("include \"{}\"", import.module)
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
            // Type-related
            "type", "container_type", "definition_type",
            // Identifiers
            "identifier", "typedef_identifier", "annotation_identifier",
            // Definitions
            "const_definition", "union_definition", "exception_definition",
            "senum_definition", "interaction_definition", "annotation_definition",
            "fb_annotation_definition",
            // Declarations
            "namespace_declaration", "package_declaration",
            // Modifiers
            "function_modifier", "field_modifier", "exception_modifier",
            // Other
            "throws", "struct_literal",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "enum_definition",
            "function_definition",
            "service_definition",
            "struct_definition",
            "typedef_definition",
            "include_statement",
        ];
        validate_unused_kinds_audit(&Thrift, documented_unused)
            .expect("Thrift unused node kinds audit failed");
    }
}
