//! YAML language support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// YAML language support.
pub struct Yaml;

impl Language for Yaml {
    fn name(&self) -> &'static str {
        "YAML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }
    fn grammar_name(&self) -> &'static str {
        "yaml"
    }

    fn has_symbols(&self) -> bool {
        false
    }

    // YAML is data, not code - no functions/types/control flow

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
        if node.kind() == "block_mapping_pair" {
            let key = node.child_by_field_name("key")?;
            let key_text = &content[key.byte_range()];

            return Some(Symbol {
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
            });
        }
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // YAML has no imports
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
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block_mapping_pair", "block_node", "block_scalar",
            "block_sequence", "block_sequence_item",
        ];
        validate_unused_kinds_audit(&Yaml, documented_unused)
            .expect("YAML unused node kinds audit failed");
    }
}
