//! RON (Rusty Object Notation) support.

use crate::{ContainerBody, Language, LanguageSymbols};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
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

impl LanguageSymbols for Ron {}

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
            // structural node, not extracted as symbols
            "struct",
            "struct_entry",
        ];
        validate_unused_kinds_audit(&Ron, documented_unused)
            .expect("RON unused node kinds audit failed");
    }
}
