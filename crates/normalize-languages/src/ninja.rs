//! Ninja build system support.

use crate::{Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Ninja language support.
pub struct Ninja;

impl Language for Ninja {
    fn name(&self) -> &'static str {
        "Ninja"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ninja"]
    }
    fn grammar_name(&self) -> &'static str {
        "ninja"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        match node.kind() {
            "include" | "subninja" => {
                let text = &content[node.byte_range()];
                vec![Import {
                    module: text.trim().to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line: node.start_position().row + 1,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Ninja: subninja or include
        format!("include {}", import.module)
    }
}

impl LanguageSymbols for Ninja {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "manifest", "identifier", "body",
        ];
        validate_unused_kinds_audit(&Ninja, documented_unused)
            .expect("Ninja unused node kinds audit failed");
    }
}
