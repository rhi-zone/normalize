//! JSON language support.

use crate::{Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// JSON language support.
pub struct Json;

impl Language for Json {
    fn name(&self) -> &'static str {
        "JSON"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }
    fn grammar_name(&self) -> &'static str {
        "json"
    }

    fn has_symbols(&self) -> bool {
        false
    }

    // JSON is data, not code - no functions/types/control flow
    // "pair" nodes are key-value pairs that we extract as symbols

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        // Extract JSON key-value pairs as symbols
        // node.kind() is already "pair" from container_kinds()
        let key = node.child_by_field_name("key")?;
        let key_text = content[key.byte_range()].trim_matches('"');

        Some(Symbol {
            name: key_text.to_string(),
            kind: SymbolKind::Variable,
            signature: key_text.to_string(),
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

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // JSON has no "interesting" unused kinds matching our patterns
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Json, documented_unused)
            .expect("JSON unused node kinds audit failed");
    }
}
