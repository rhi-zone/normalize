//! Nginx configuration file support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["block"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["directive"] // include directive
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["block", "directive"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "block" {
            return Vec::new();
        }

        if let Some(name) = self.node_name(node, content) {
            return vec![Export {
                name: name.to_string(),
                kind: SymbolKind::Module,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["block"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["block_directive"] // if blocks are block_directives
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["block_directive"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["block", "block_directive"]
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
        if node.kind() != "block" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
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

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
        ];
        validate_unused_kinds_audit(&Nginx, documented_unused)
            .expect("Nginx unused node kinds audit failed");
    }
}
