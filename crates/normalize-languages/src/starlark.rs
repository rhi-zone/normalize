//! Starlark (Bazel/Buck) support.

use crate::{ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "function_definition" {
            return Vec::new();
        }

        if let Some(name) = self.node_name(node, content) {
            return vec![Export {
                name: name.to_string(),
                kind: SymbolKind::Function,
                line: node.start_position().row + 1,
            }];
        }
        Vec::new()
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "function_definition" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
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
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[]
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
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
