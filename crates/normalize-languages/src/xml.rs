//! XML language support.

use crate::{Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// XML language support.
pub struct Xml;

impl Language for Xml {
    fn name(&self) -> &'static str {
        "XML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["xml", "xsl", "xslt", "xsd", "svg", "plist"]
    }
    fn grammar_name(&self) -> &'static str {
        "xml"
    }

    fn has_symbols(&self) -> bool {
        false
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
        if node.kind() != "element" {
            return None;
        }

        // Find the tag name from start_tag
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "tag_name" {
                        let name = content[inner.byte_range()].to_string();
                        return Some(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Module,
                            signature: format!("<{}>", name),
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
                }
            }
        }
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
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
            "Enumeration", "NotationType", "StringType", "TokenizedType",
            "doctypedecl",
        ];
        validate_unused_kinds_audit(&Xml, documented_unused)
            .expect("XML unused node kinds audit failed");
    }
}
