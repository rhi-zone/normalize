//! Emacs Lisp language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Emacs Lisp language support.
pub struct Elisp;

impl Language for Elisp {
    fn name(&self) -> &'static str {
        "Emacs Lisp"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["el", "elc"]
    }
    fn grammar_name(&self) -> &'static str {
        "elisp"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        if let Some(rest) = text.strip_prefix("(require ") {
            let module = rest
                .split(|c: char| c.is_whitespace() || c == ')')
                .next()
                .map(|s| s.trim_matches('\''))
                .unwrap_or("")
                .to_string();

            if !module.is_empty() {
                return vec![Import {
                    module,
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Emacs Lisp: (require 'package)
        format!("(require '{})", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.contains("--") {
            Visibility::Private
        } else {
            Visibility::Public
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

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Elisp, documented_unused)
            .expect("Emacs Lisp unused node kinds audit failed");
    }
}
