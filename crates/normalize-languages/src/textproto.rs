//! Protocol Buffers text format support.

use crate::{Import, Language, Symbol, Visibility};
use tree_sitter::Node;

/// TextProto language support.
pub struct TextProto;

impl Language for TextProto {
    fn name(&self) -> &'static str {
        "TextProto"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["textproto", "pbtxt"]
    }
    fn grammar_name(&self) -> &'static str {
        "textproto"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }
    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Text proto has no imports
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
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
            "identifier", "type_name", "signed_identifier",
        ];
        validate_unused_kinds_audit(&TextProto, documented_unused)
            .expect("TextProto unused node kinds audit failed");
    }
}
