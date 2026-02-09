//! GraphQL language support.

use crate::{
    Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism, simple_symbol,
};
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

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "object_type_definition",
            "interface_type_definition",
            "enum_type_definition",
            "union_type_definition",
            "input_object_type_definition",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["field_definition", "operation_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[
            "object_type_definition",
            "interface_type_definition",
            "enum_type_definition",
            "union_type_definition",
            "input_object_type_definition",
            "scalar_type_definition",
        ]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "object_type_definition",
            "interface_type_definition",
            "operation_definition",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NotApplicable
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "object_type_definition" => SymbolKind::Struct,
            "interface_type_definition" => SymbolKind::Interface,
            "enum_type_definition" | "union_type_definition" => SymbolKind::Enum,
            "input_object_type_definition" => SymbolKind::Struct,
            "scalar_type_definition" => SymbolKind::Type,
            "operation_definition" => SymbolKind::Function,
            "field_definition" => SymbolKind::Method,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["selection_set"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["selection_set"]
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["selection_set", "object_type_definition"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_symbol(
            node,
            content,
            name,
            SymbolKind::Method,
            self.extract_docstring(node, content),
        ))
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
            docstring: self.extract_docstring(node, content),
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // GraphQL uses """ for descriptions
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "description" || text.starts_with("\"\"\"") {
                let inner = text
                    .trim_start_matches("\"\"\"")
                    .trim_end_matches("\"\"\"")
                    .trim();
                if !inner.is_empty() {
                    return Some(inner.to_string());
                }
            }
            prev = sibling.prev_sibling();
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // GraphQL has no imports
        String::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("fields_definition")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
        ];
        validate_unused_kinds_audit(&GraphQL, documented_unused)
            .expect("GraphQL unused node kinds audit failed");
    }
}
