//! Cap'n Proto schema support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Cap'n Proto language support.
pub struct Capnp;

impl Language for Capnp {
    fn name(&self) -> &'static str {
        "Cap'n Proto"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["capnp"]
    }
    fn grammar_name(&self) -> &'static str {
        "capnp"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
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
        // Cap'n Proto: using import "file.capnp"
        format!("using import \"{}\"", import.module)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
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
            "type_identifier", "type_definition", "primitive_type", "list_type",
            "custom_type", "field_type", "extend_type",
            // Method-related
            "method_identifier", "method_parameters", "return_type", "return_identifier",
            "named_return_type", "named_return_types", "unnamed_return_type", "param_identifier",
            // Struct/enum-related
            "nested_struct", "nested_enum", "enum_field", "enum_member", "enum_identifier",
            "field_identifier", "struct_shorthand",
            // Import-related
            "import_path", "import_using",
            // Other
            "const_identifier", "generic_identifier", "annotation_identifier",
            "annotation_definition_identifier", "unique_id_statement",
            "top_level_annotation_body", "block_text",
            // covered by tags.scm
            "enum",
            "interface",
            "method",
            "struct",
            "import",
        ];
        validate_unused_kinds_audit(&Capnp, documented_unused)
            .expect("Cap'n Proto unused node kinds audit failed");
    }
}
