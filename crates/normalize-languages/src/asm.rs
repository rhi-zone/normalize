//! Assembly language support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Assembly language support.
pub struct Asm;

impl Language for Asm {
    fn name(&self) -> &'static str {
        "Assembly"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["asm", "s", "S"]
    }
    fn grammar_name(&self) -> &'static str {
        "asm"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "label" {
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
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new() // asm grammar doesn't have imports
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Assembly has no standard import mechanism
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // Labels end with ':'
        let text = &content[node.byte_range()];
        let name = text.trim().trim_end_matches(':');
        if !name.is_empty() { Some(name) } else { None }
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
            // Asm instructions are too granular for symbol extraction
            "instruction",
            // Comments
            "block_comment",
        ];
        validate_unused_kinds_audit(&Asm, documented_unused)
            .expect("Assembly unused node kinds audit failed");
    }
}
