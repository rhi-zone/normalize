//! Common Lisp language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Common Lisp language support.
pub struct CommonLisp;

impl Language for CommonLisp {
    fn name(&self) -> &'static str {
        "Common Lisp"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["lisp", "lsp", "cl", "asd"]
    }
    fn grammar_name(&self) -> &'static str {
        "commonlisp"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list_lit" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        for prefix in &["(require ", "(use-package ", "(ql:quickload "] {
            if let Some(rest) = text.strip_prefix(prefix) {
                let module = rest
                    .split(|c: char| c.is_whitespace() || c == ')')
                    .next()
                    .map(|s| s.trim_matches(|c| c == '\'' || c == ':' || c == '"'))
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
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Common Lisp: (use-package :package) or (use-package :package (:import-from #:a #:b))
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(use-package :{})", import.module)
        } else {
            let symbols: Vec<String> = names_to_use.iter().map(|n| format!("#:{}", n)).collect();
            format!(
                "(use-package :{} (:import-from {}))",
                import.module,
                symbols.join(" ")
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
        // list/list_lit is itself "( ... )" — use node directly for paren analysis
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

impl LanguageSymbols for CommonLisp {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Loop-related clauses
            "accumulation_clause", "condition_clause", "do_clause", "for_clause",
            "for_clause_word", "loop_clause", "loop_macro", "repeat_clause",
            "termination_clause", "while_clause", "with_clause",
            // Format string specifiers
            "format_directive_type", "format_modifiers", "format_prefix_parameters",
            "format_specifier",
            // Comments
            "block_comment",
        ];
        validate_unused_kinds_audit(&CommonLisp, documented_unused)
            .expect("Common Lisp unused node kinds audit failed");
    }
}
