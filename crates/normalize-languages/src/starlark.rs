//! Starlark (Bazel/Buck) support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Starlark language support.
pub struct Starlark;

impl Language for Starlark {
    fn name(&self) -> &'static str {
        "Starlark"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["star", "bzl"]
    }
    fn grammar_name(&self) -> &'static str {
        "starlark"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "load_statement" {
            return Vec::new();
        }

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

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Starlark: load("//path", "name")
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("load(\"{}\")", import.module)
        } else {
            let quoted: Vec<String> = names_to_use.iter().map(|n| format!("\"{}\"", n)).collect();
            format!("load(\"{}\", {})", import.module, quoted.join(", "))
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
            // Blocks and modules
            "block", "module",
            // Statements
            "pass_statement", "break_statement", "continue_statement", "return_statement",
            "expression_statement", "else_clause", "elif_clause", "if_clause", "for_in_clause",
            // Expressions
            "expression", "primary_expression", "parenthesized_expression",
            "binary_operator", "boolean_operator", "comparison_operator",
            "unary_operator", "not_operator",
            // Comprehensions
            "list_comprehension", "dictionary_comprehension",
            // Lambda
            "lambda", "lambda_parameters",
            // Other
            "identifier",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "for_statement",
            "load_statement",
            "if_statement",
            "conditional_expression",
        ];
        validate_unused_kinds_audit(&Starlark, documented_unused)
            .expect("Starlark unused node kinds audit failed");
    }
}
