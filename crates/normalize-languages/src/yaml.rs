//! YAML language support.

use crate::{Language, LanguageSymbols};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        // Pairs with block_mapping values are containers
        if node.kind() == "block_mapping_pair"
            && let Some(value) = node.child_by_field_name("value")
        {
            // value is either flow_node (scalar) or block_node > block_mapping
            if value.kind() == "block_node" {
                let mut cursor = value.walk();
                for child in value.children(&mut cursor) {
                    if child.kind() == "block_mapping" {
                        return crate::SymbolKind::Module;
                    }
                }
            }
        }
        tag_kind
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if node.kind() == "block_mapping_pair"
            && let Some(key) = node.child_by_field_name("key")
        {
            return find_scalar_text(key, content);
        }
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "block_mapping_pair"
            && let Some(value) = node.child_by_field_name("value")
            && value.kind() == "block_node"
        {
            let mut cursor = value.walk();
            for child in value.children(&mut cursor) {
                if child.kind() == "block_mapping" {
                    return Some(child);
                }
            }
        }
        None
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(key) = self.node_name(node, content) {
            if let Some(value) = node.child_by_field_name("value") {
                if value.kind() == "block_node" {
                    return format!("{}:", key);
                }
                let val_text = content[value.byte_range()].trim();
                if val_text.len() > 40 {
                    return format!("{}: {}…", key, &val_text[..37]);
                }
                return format!("{}: {}", key, val_text);
            }
            return key.to_string();
        }
        content[node.byte_range()]
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

impl LanguageSymbols for Yaml {}

/// Walk into nested scalar nodes to find the text content.
fn find_scalar_text<'a>(node: Node, content: &'a str) -> Option<&'a str> {
    let kind = node.kind();
    if kind == "string_scalar" || kind == "string_content" {
        return Some(&content[node.byte_range()]);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(text) = find_scalar_text(child, content) {
            return Some(text);
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
            "block_node", "block_scalar",
            "block_sequence", "block_sequence_item",
            // structural node, not extracted as symbols
            "block_mapping",
        ];
        validate_unused_kinds_audit(&Yaml, documented_unused)
            .expect("YAML unused node kinds audit failed");
    }
}
