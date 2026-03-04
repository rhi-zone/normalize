//! GraphQL language support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility, simple_symbol};
use tree_sitter::Node;

/// GraphQL language support.
pub struct GraphQL;

impl GraphQL {
    /// Recursively collect named_type names from implements_interfaces.
    /// The node is recursive: implements_interfaces > implements_interfaces > named_type > name
    fn collect_named_types(node: &Node, out: &mut Vec<String>, content: &str) {
        if node.kind() == "named_type" {
            // Get the name child (first named child)
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i as u32)
                    && child.is_named()
                {
                    out.push(content[child.byte_range()].to_string());
                    return;
                }
            }
            return;
        }
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32) {
                Self::collect_named_types(&child, out, content);
            }
        }
    }
}

impl Language for GraphQL {
    fn name(&self) -> &'static str {
        "GraphQL"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["graphql", "gql"]
    }
    fn grammar_name(&self) -> &'static str {
        "graphql"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_symbol(node, content, name, SymbolKind::Method, None))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "interface_type_definition" => (SymbolKind::Interface, "interface"),
            "enum_type_definition" => (SymbolKind::Enum, "enum"),
            "union_type_definition" => (SymbolKind::Enum, "union"),
            "input_object_type_definition" => (SymbolKind::Struct, "input"),
            "scalar_type_definition" => (SymbolKind::Type, "scalar"),
            _ => (SymbolKind::Struct, "type"),
        };

        // Extract implements_interfaces (recursive: implements_interfaces > named_type > name)
        let mut implements = Vec::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "implements_interfaces"
            {
                Self::collect_named_types(&child, &mut implements, content);
            }
        }

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements,
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // GraphQL has no imports
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("fields_definition")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // fields_definition: "{ field1: Type\n  field2: Type\n }"
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(n) = node.child_by_field_name("name") {
            return Some(&content[n.byte_range()]);
        }
        // Fallback: find first child of kind "name"
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "name"
            {
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
        // Run cross_check_node_kinds to populate - many kinds already used
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "argument", "directive", "enum_value", "enum_value_definition",
            "enum_values_definition", "executable_definition", "field",
            "fields_definition", "fragment_definition", "fragment_spread",
            "implements_interfaces", "inline_fragment", "input_fields_definition",
            "input_value_definition", "named_type", "type", "type_condition",
            "type_definition", "type_extension", "type_system_definition",
            "type_system_extension", "union_member_types", "variable_definition",
            "arguments_definition", "definition", "directive_definition", "list_type",
            "non_null_type", "object_type_extension", "operation_type",
            "root_operation_type_definition", "scalar_type_extension", "schema_definition",
            "enum_type_extension", "input_object_type_extension", "interface_type_extension",
            "type_system_directive_location", "union_type_extension", "variable_definitions",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "scalar_type_definition",
            "union_type_definition",
            "field_definition",
            "input_object_type_definition",
            "enum_type_definition",
        ];
        validate_unused_kinds_audit(&GraphQL, documented_unused)
            .expect("GraphQL unused node kinds audit failed");
    }
}
