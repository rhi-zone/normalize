//! RON (Rusty Object Notation) support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// RON language support.
pub struct Ron;

impl Language for Ron {
    fn name(&self) -> &'static str {
        "RON"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ron"]
    }
    fn grammar_name(&self) -> &'static str {
        "ron"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn signature_suffix(&self) -> &'static str {
        ""
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
        if node.kind() != "struct" && node.kind() != "map" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Struct,
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
        if node.kind() != "struct" {
            return None;
        }
        self.extract_container(node, content)
    }

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // RON has no imports
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[]
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // RON struct uses ( ... ), map uses { ... }; use node itself for body analysis
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        match body_node.kind() {
            "struct" => crate::body::analyze_paren_body(body_node, content, inner_indent),
            "map" => crate::body::analyze_brace_body(body_node, content, inner_indent),
            _ => None,
        }
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
            "identifier", "struct_name", "unit_struct", "enum_variant",
            "map_entry", "block_comment",
        ];
        validate_unused_kinds_audit(&Ron, documented_unused)
            .expect("RON unused node kinds audit failed");
    }
}
