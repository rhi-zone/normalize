//! Scheme language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Scheme language support.
pub struct Scheme;

impl Language for Scheme {
    fn name(&self) -> &'static str {
        "Scheme"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scm", "ss", "rkt"]
    }
    fn grammar_name(&self) -> &'static str {
        "scheme"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        for prefix in &["(import ", "(require "] {
            if text.starts_with(prefix) {
                return vec![Import {
                    module: "import".to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Scheme: (import (library)) or (import (only (library) a b c))
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(import ({}))", import.module)
        } else {
            format!(
                "(import (only ({}) {}))",
                import.module,
                names_to_use.join(" ")
            )
        }
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
        // list is itself "( ... )" — use node directly for paren analysis
        Some(*node)
    }
    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_paren_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // The @definition.* captures a list for (define ...) forms.
        // Two cases:
        // 1. (define (name args) body) — second named-child is a list; its first
        //    symbol child is the function name.
        // 2. (define name ...) — second named-child is a symbol (the name directly).
        if node.kind() != "list" {
            return node
                .child_by_field_name("name")
                .map(|n| &content[n.byte_range()]);
        }
        let mut cursor = node.walk();
        let mut seen_define = false;
        for child in node.children(&mut cursor) {
            match child.kind() {
                "symbol" if !seen_define => {
                    // First symbol is the form keyword (define, define-syntax, etc.)
                    seen_define = true;
                }
                "symbol" if seen_define => {
                    // Second symbol: (define name ...)
                    return Some(&content[child.byte_range()]);
                }
                "list" if seen_define => {
                    // Second child is a nested list: (define (name args) ...) form
                    // The name is the first symbol inside this nested list.
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "symbol" {
                            return Some(&content[inner.byte_range()]);
                        }
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }
}

impl LanguageSymbols for Scheme {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block_comment",
        ];
        validate_unused_kinds_audit(&Scheme, documented_unused)
            .expect("Scheme unused node kinds audit failed");
    }
}
