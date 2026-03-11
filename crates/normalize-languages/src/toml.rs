//! TOML language support.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// TOML language support.
pub struct Toml;

impl Language for Toml {
    fn name(&self) -> &'static str {
        "TOML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }
    fn grammar_name(&self) -> &'static str {
        "toml"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        match node.kind() {
            "table" | "array_table" => {
                // First bare_key child is the section name
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "bare_key" || child.kind() == "quoted_key" {
                        return Some(&content[child.byte_range()]);
                    }
                }
                None
            }
            "pair" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "bare_key" || child.kind() == "quoted_key" {
                        return Some(&content[child.byte_range()]);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(key) = self.node_name(node, content) {
            match node.kind() {
                "table" | "array_table" => {
                    return format!("[{}]", key);
                }
                "pair" => {
                    // Find value child (after the = sign)
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        let k = child.kind();
                        if k != "bare_key" && k != "quoted_key" && k != "=" {
                            let val_text = &content[child.byte_range()];
                            if val_text.len() > 40 {
                                return format!("{} = {}…", key, &val_text[..37]);
                            }
                            return format!("{} = {}", key, val_text);
                        }
                    }
                    return key.to_string();
                }
                _ => {}
            }
        }
        content[node.byte_range()]
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

impl LanguageSymbols for Toml {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // TOML has no "interesting" unused kinds matching our patterns
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Toml, documented_unused)
            .expect("TOML unused node kinds audit failed");
    }
}
