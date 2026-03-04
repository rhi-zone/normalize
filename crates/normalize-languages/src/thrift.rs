//! Apache Thrift IDL support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
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

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "function_definition" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: text.trim().to_string(),
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

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let kind = match node.kind() {
            "struct_definition" => SymbolKind::Struct,
            "service_definition" => SymbolKind::Interface,
            "enum_definition" => SymbolKind::Enum,
            _ => return None,
        };

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind,
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

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
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
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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
            "include_statement",
        ];
        validate_unused_kinds_audit(&Thrift, documented_unused)
            .expect("Thrift unused node kinds audit failed");
    }
}
