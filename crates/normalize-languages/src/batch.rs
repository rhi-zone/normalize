//! Windows Batch file support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Batch: call script.bat
        format!("call {}", import.module)
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
