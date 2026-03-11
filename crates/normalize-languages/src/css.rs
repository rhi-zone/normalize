//! CSS language support with symbol extraction.
//!
//! CSS symbols: rule_set (selectors → Class), media/supports/keyframes → Module,
//! declarations → Variable. Nested rule_sets inside at-rules become children.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// CSS language support.
pub struct Css;

impl Language for Css {
    fn name(&self) -> &'static str {
        "CSS"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["css"]
    }
    fn grammar_name(&self) -> &'static str {
        "css"
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
        match node.kind() {
            // At-rules containing blocks are containers
            "media_statement" | "supports_statement" | "keyframes_statement" => {
                crate::SymbolKind::Module
            }
            _ => tag_kind,
        }
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        match node.kind() {
            "rule_set" => {
                // Extract the selectors text
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "selectors" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            "media_statement" => {
                // Extract feature_query or keyword after @media
                extract_at_rule_name(node, content, "@media")
            }
            "supports_statement" => extract_at_rule_name(node, content, "@supports"),
            "keyframes_statement" => {
                // keyframes_name child
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "keyframes_name" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            "declaration" => {
                // property_name child
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "property_name" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        match node.kind() {
            "rule_set" | "media_statement" | "supports_statement" | "keyframes_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" || child.kind() == "keyframe_block_list" {
                        return Some(child);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(name) = self.node_name(node, content) {
            match node.kind() {
                "rule_set" => format!("{} {{ … }}", name),
                "media_statement" => format!("@media {} {{ … }}", name),
                "supports_statement" => format!("@supports {} {{ … }}", name),
                "keyframes_statement" => format!("@keyframes {} {{ … }}", name),
                "declaration" => {
                    // Find value after property_name
                    let mut cursor = node.walk();
                    let mut found_name = false;
                    for child in node.children(&mut cursor) {
                        if child.kind() == "property_name" {
                            found_name = true;
                        } else if found_name && child.kind() != ":" && child.kind() != ";" {
                            let val = content[child.byte_range()].trim();
                            if val.len() > 40 {
                                return format!("{}: {}…", name, &val[..37]);
                            }
                            return format!("{}: {}", name, val);
                        }
                    }
                    name.to_string()
                }
                _ => name.to_string(),
            }
        } else {
            content[node.byte_range()]
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        }
    }
}

impl LanguageSymbols for Css {}

/// Extract the text between an at-rule keyword and its block.
fn extract_at_rule_name<'a>(node: &Node, content: &'a str, keyword: &str) -> Option<&'a str> {
    let full = &content[node.byte_range()];
    let after_keyword = full.strip_prefix(keyword)?.trim_start();
    // Take everything up to the opening brace
    let name = after_keyword.split('{').next()?.trim();
    if name.is_empty() {
        return None;
    }
    // Find the offset within the node and return a reference into content
    let start = node.start_byte() + full.find(name)?;
    let end = start + name.len();
    Some(&content[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "binary_expression", "block", "call_expression", "charset_statement",
            "class_name", "class_selector",
            "function_name",
            "identifier", "import_statement", "important", "important_value",
            "keyframe_block", "keyframe_block_list",
            "namespace_statement", "postcss_statement",
            "pseudo_class_selector", "scope_statement",
        ];
        validate_unused_kinds_audit(&Css, documented_unused)
            .expect("CSS unused node kinds audit failed");
    }
}
