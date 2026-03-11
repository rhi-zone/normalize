//! JSON language support.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// JSON language support.
pub struct Json;

impl Language for Json {
    fn name(&self) -> &'static str {
        "JSON"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }
    fn grammar_name(&self) -> &'static str {
        "json"
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
        // Pairs with object values act as containers (sections/namespaces)
        if node.kind() == "pair"
            && let Some(value) = node.child_by_field_name("value")
            && value.kind() == "object"
        {
            return crate::SymbolKind::Module;
        }
        tag_kind
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // For pair nodes, extract the key string content
        if node.kind() == "pair"
            && let Some(key) = node.child_by_field_name("key")
        {
            let mut cursor = key.walk();
            for child in key.children(&mut cursor) {
                if child.kind() == "string_content" {
                    return Some(&content[child.byte_range()]);
                }
            }
        }
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "pair"
            && let Some(value) = node.child_by_field_name("value")
            && value.kind() == "object"
        {
            return Some(value);
        }
        None
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if node.kind() == "pair"
            && let Some(key) = self.node_name(node, content)
        {
            if let Some(value) = node.child_by_field_name("value") {
                return match value.kind() {
                    "object" => format!("{}: {{}}", key),
                    "array" => format!("{}: []", key),
                    _ => {
                        let val_text = &content[value.byte_range()];
                        if val_text.len() > 40 {
                            format!("{}: {}…", key, &val_text[..37])
                        } else {
                            format!("{}: {}", key, val_text)
                        }
                    }
                };
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

impl LanguageSymbols for Json {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // JSON has no "interesting" unused kinds matching our patterns
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Json, documented_unused)
            .expect("JSON unused node kinds audit failed");
    }
}
