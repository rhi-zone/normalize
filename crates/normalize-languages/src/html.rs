//! HTML language support (parse only, minimal skeleton).

use crate::{Language, Symbol};
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

    fn has_symbols(&self) -> bool {
        false
    }

    // HTML has no functions/containers/types in the traditional sense

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }

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

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
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
