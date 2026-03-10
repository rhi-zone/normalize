//! MATLAB language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// MATLAB language support.
pub struct Matlab;

impl Language for Matlab {
    fn name(&self) -> &'static str {
        "MATLAB"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["m"]
    }
    fn grammar_name(&self) -> &'static str {
        "matlab"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("import ") {
            return Vec::new();
        }

        vec![Import {
            module: text[7..].trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: text.contains('*'),
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // MATLAB: import package.* or import package.function
        if import.is_wildcard {
            format!("import {}.*", import.module)
        } else {
            format!("import {}", import.module)
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
        // MATLAB class_definition has no dedicated body field; use node itself
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // classdef Foo\n  methods...\nend — skip first line, strip `end`
        crate::body::analyze_keyword_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }
}

impl LanguageSymbols for Matlab {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Operators
            "binary_operator", "boolean_operator", "comparison_operator", "global_operator",
            "handle_operator", "metaclass_operator", "not_operator", "persistent_operator",
            "postfix_operator", "spread_operator", "unary_operator",
            // Statements
            "arguments_statement", "break_statement", "continue_statement", "return_statement",
            "spmd_statement",
            // Control flow clauses
            "case_clause", "else_clause", "elseif_clause", "otherwise_clause",
            // Class-related
            "class_property", "enum", "enumeration", "superclass", "superclasses",
            // Function-related
            "block", "field_expression", "formatting_sequence", "function_arguments",
            "function_call", "function_output", "function_signature", "identifier", "lambda",
            "parfor_options", "validation_functions",
            // control flow — not extracted as symbols
            "if_statement",
            "catch_clause",
            "switch_statement",
            "while_statement",
            "for_statement",
            "try_statement",
            "methods",
        ];
        validate_unused_kinds_audit(&Matlab, documented_unused)
            .expect("MATLAB unused node kinds audit failed");
    }
}
