//! Zsh language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Zsh language support.
pub struct Zsh;

impl Language for Zsh {
    fn name(&self) -> &'static str {
        "Zsh"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zsh", "zshrc", "zshenv", "zprofile"]
    }
    fn grammar_name(&self) -> &'static str {
        "zsh"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // source file or . file
        let module = text
            .strip_prefix("source ")
            .or_else(|| text.strip_prefix(". "))
            .map(|rest| rest.trim().to_string());

        if let Some(module) = module {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: true,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Zsh: source file or . file
        format!("source {}", import.module)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
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
            "else_clause",
            // control flow — not extracted as symbols
            "case_item",
            "if_statement",
            "elif_clause",
            "while_statement",
            "for_statement",
            "case_statement",
        ];
        validate_unused_kinds_audit(&Zsh, documented_unused)
            .expect("Zsh unused node kinds audit failed");
    }
}
