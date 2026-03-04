//! Bash language support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
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

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: format!("function {}", name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Bash: source file or . file
        format!("source {}", import.module)
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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
            "binary_expression", "brace_expression", "c_style_for_statement",
            "compound_statement", "declaration_command", "else_clause",
            "heredoc_body", "parenthesized_expression", "postfix_expression",
            "redirected_statement", "ternary_expression", "test_operator",
            "unary_expression",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
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
