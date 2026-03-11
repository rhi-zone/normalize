//! XML language support with symbol extraction.
//!
//! XML elements are extracted as symbols: elements with child elements become
//! Modules (containers), leaf elements become Variables. Tag name is the symbol name.

use crate::{Language, LanguageSymbols};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        if node.kind() == "element" && has_child_elements(node) {
            return crate::SymbolKind::Module;
        }
        tag_kind
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if node.kind() == "element" {
            return extract_xml_tag_name(node, content);
        }
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "element" && has_child_elements(node) {
            // Return the content node which contains child elements
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "content" {
                    return Some(child);
                }
            }
        }
        None
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(tag) = self.node_name(node, content) {
            if let Some(attrs) = extract_key_attributes(node, content) {
                return format!("<{} {}>", tag, attrs);
            }
            return format!("<{}>", tag);
        }
        content[node.byte_range()]
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

impl LanguageSymbols for Xml {}

/// Check if an element has child elements in its content.
fn has_child_elements(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "content" {
            let mut inner = child.walk();
            for grandchild in child.children(&mut inner) {
                if grandchild.kind() == "element" {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract tag name from STag or EmptyElemTag.
fn extract_xml_tag_name<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "STag" || child.kind() == "EmptyElemTag" {
            let mut inner = child.walk();
            for part in child.children(&mut inner) {
                if part.kind() == "Name" {
                    return Some(&content[part.byte_range()]);
                }
            }
        }
    }
    None
}

/// Extract key attributes for the signature.
fn extract_key_attributes(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "STag" || child.kind() == "EmptyElemTag" {
            let mut parts = Vec::new();
            let mut inner = child.walk();
            for attr in child.children(&mut inner) {
                if attr.kind() == "Attribute" {
                    let mut attr_cursor = attr.walk();
                    let mut attr_name = None;
                    let mut attr_val = None;
                    for part in attr.children(&mut attr_cursor) {
                        if part.kind() == "Name" {
                            attr_name = Some(&content[part.byte_range()]);
                        } else if part.kind() == "AttValue" {
                            attr_val = Some(&content[part.byte_range()]);
                        }
                    }
                    if let (Some(name), Some(val)) = (attr_name, attr_val)
                        && (name == "id" || name == "class" || name == "name")
                    {
                        parts.push(format!("{}={}", name, val));
                    }
                }
            }
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
    }
    None
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
