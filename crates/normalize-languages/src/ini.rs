//! INI configuration file support.

use crate::{Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// INI language support.
pub struct Ini;

impl Language for Ini {
    fn name(&self) -> &'static str {
        "INI"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ini", "cfg", "conf", "properties"]
    }
    fn grammar_name(&self) -> &'static str {
        "ini"
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
        if node.kind() != "section" {
            return None;
        }

        let name = self.node_name(node, content)?;

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
            signature: format!("[{}]", name),
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
            .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Ini, documented_unused)
            .expect("INI unused node kinds audit failed");
    }
}
