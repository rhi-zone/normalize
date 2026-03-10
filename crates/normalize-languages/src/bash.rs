//! Bash language support.

use crate::{Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Bash language support.
pub struct Bash;

impl Language for Bash {
    fn name(&self) -> &'static str {
        "Bash"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash"]
    }
    fn grammar_name(&self) -> &'static str {
        "bash"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        format!("function {}", name)
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Bash: source file or . file
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

impl LanguageSymbols for Bash {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "binary_expression", "brace_expression", "c_style_for_statement",
            "compound_statement", "declaration_command", "else_clause",
            "heredoc_body", "parenthesized_expression", "postfix_expression",
            "redirected_statement", "ternary_expression", "test_operator",
            "unary_expression",
            // control flow — not extracted as symbols
            "if_statement",
            "for_statement",
            "case_statement",
            "case_item",
            "while_statement",
            "elif_clause",
        ];
        validate_unused_kinds_audit(&Bash, documented_unused)
            .expect("Bash unused node kinds audit failed");
    }
}
