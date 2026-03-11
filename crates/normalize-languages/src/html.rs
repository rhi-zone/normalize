//! HTML language support with symbol extraction.
//!
//! HTML elements are extracted as symbols: elements with child elements become
//! Modules (containers), leaf elements become Variables. Tag name is the symbol name.

use crate::{Language, LanguageEmbedded, LanguageSymbols};
use tree_sitter::Node;

/// HTML language support.
pub struct Html;

impl Language for Html {
    fn name(&self) -> &'static str {
        "HTML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
    }
    fn grammar_name(&self) -> &'static str {
        "html"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn as_embedded(&self) -> Option<&dyn LanguageEmbedded> {
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
        if node.kind() == "element"
            || node.kind() == "script_element"
            || node.kind() == "style_element"
        {
            return extract_html_tag_name(node, content);
        }
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // For elements with children, the element itself is the container body
        // (child elements are direct children of the element node)
        if node.kind() == "element" && has_child_elements(node) {
            return Some(*node);
        }
        None
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(tag) = self.node_name(node, content) {
            // Include key attributes (id, class) in signature
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

impl LanguageSymbols for Html {}

impl LanguageEmbedded for Html {
    fn embedded_content(&self, node: &Node, content: &str) -> Option<crate::EmbeddedBlock> {
        match node.kind() {
            "script_element" => {
                let raw = find_raw_text_child(node)?;
                let grammar = detect_script_type(node, content);
                Some(crate::EmbeddedBlock {
                    grammar,
                    content: content[raw.byte_range()].to_string(),
                    start_line: raw.start_position().row + 1,
                })
            }
            "style_element" => {
                let raw = find_raw_text_child(node)?;
                Some(crate::EmbeddedBlock {
                    grammar: "css",
                    content: content[raw.byte_range()].to_string(),
                    start_line: raw.start_position().row + 1,
                })
            }
            _ => None,
        }
    }
}

/// Check if an element has child elements (not just text).
fn has_child_elements(node: &Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| {
        child.kind() == "element"
            || child.kind() == "script_element"
            || child.kind() == "style_element"
    })
}

/// Extract tag name from start_tag or self_closing_tag.
fn extract_html_tag_name<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut inner = child.walk();
            for part in child.children(&mut inner) {
                if part.kind() == "tag_name" {
                    return Some(&content[part.byte_range()]);
                }
            }
        }
    }
    None
}

/// Extract id and class attributes for the signature.
fn extract_key_attributes(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut parts = Vec::new();
            let mut inner = child.walk();
            for attr in child.children(&mut inner) {
                if attr.kind() == "attribute" {
                    let mut attr_cursor = attr.walk();
                    let mut attr_name = None;
                    let mut attr_val = None;
                    for part in attr.children(&mut attr_cursor) {
                        if part.kind() == "attribute_name" {
                            attr_name = Some(&content[part.byte_range()]);
                        } else if part.kind() == "quoted_attribute_value" {
                            attr_val = Some(&content[part.byte_range()]);
                        }
                    }
                    if let (Some(name), Some(val)) = (attr_name, attr_val)
                        && (name == "id" || name == "class")
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

/// Find the raw_text child of a script/style element.
fn find_raw_text_child<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|&child| child.kind() == "raw_text")
}

/// Detect script type from the type attribute (e.g., <script type="module">).
/// HTML scripts default to JavaScript; type="module" is still JavaScript.
fn detect_script_type(node: &Node, content: &str) -> &'static str {
    if let Some(script_type) = get_type_attribute(node, content) {
        match script_type {
            "text/typescript" => return "typescript",
            "module" | "text/javascript" | "application/javascript" => return "javascript",
            _ => {}
        }
    }
    "javascript"
}

/// Get the type attribute value from a script element.
fn get_type_attribute<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Look for start_tag which contains the attributes
        if child.kind() == "start_tag" {
            let mut inner_cursor = child.walk();
            for attr in child.children(&mut inner_cursor) {
                if attr.kind() == "attribute" {
                    // Check if this is a type attribute
                    let mut attr_cursor = attr.walk();
                    let mut is_type = false;
                    for part in attr.children(&mut attr_cursor) {
                        if part.kind() == "attribute_name" {
                            let name = &content[part.byte_range()];
                            is_type = name == "type";
                        } else if is_type && part.kind() == "quoted_attribute_value" {
                            // Get the value inside quotes
                            let value = &content[part.byte_range()];
                            return Some(value.trim_matches('"').trim_matches('\''));
                        }
                    }
                }
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
            "doctype",
        ];

        validate_unused_kinds_audit(&Html, documented_unused)
            .expect("HTML unused node kinds audit failed");
    }
}
