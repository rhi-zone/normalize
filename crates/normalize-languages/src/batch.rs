//! Windows Batch file support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Batch language support.
pub struct Batch;

impl Language for Batch {
    fn name(&self) -> &'static str {
        "Batch"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["bat", "cmd"]
    }
    fn grammar_name(&self) -> &'static str {
        "batch"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &[] // batch grammar doesn't have import nodes
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "variable_declaration"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        match node.kind() {
            "function_definition" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            "variable_declaration" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Variable,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[] // batch grammar doesn't have control flow nodes
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
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

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new() // batch grammar doesn't have import nodes
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Batch: call script.bat
        format!("call {}", import.module)
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
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
            "identifier",
        ];
        validate_unused_kinds_audit(&Batch, documented_unused)
            .expect("Batch unused node kinds audit failed");
    }
}
