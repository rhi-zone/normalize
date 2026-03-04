//! Ninja build system support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Ninja language support.
pub struct Ninja;

impl Language for Ninja {
    fn name(&self) -> &'static str {
        "Ninja"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ninja"]
    }
    fn grammar_name(&self) -> &'static str {
        "ninja"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "rule" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
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

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        match node.kind() {
            "include" | "subninja" => {
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
            _ => Vec::new(),
        }
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Ninja: subninja or include
        format!("include {}", import.module)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "manifest", "identifier", "body",
        ];
        validate_unused_kinds_audit(&Ninja, documented_unused)
            .expect("Ninja unused node kinds audit failed");
    }
}
