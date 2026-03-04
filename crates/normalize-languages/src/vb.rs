//! Visual Basic language support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Visual Basic language support.
pub struct VB;

impl Language for VB {
    fn name(&self) -> &'static str {
        "Visual Basic"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vb", "vbs"]
    }
    fn grammar_name(&self) -> &'static str {
        "vb"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        match node.kind() {
            "method_declaration" | "property_declaration" => {
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
                    visibility: self.get_visibility(node, content),
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "class_block" | "module_block" | "structure_block" | "interface_block" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    signature: first_line.trim().to_string(),
                    docstring: None,
                    attributes: Vec::new(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: self.get_visibility(node, content),
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "enum_block" | "delegate_declaration" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Type,
                    signature: first_line.trim().to_string(),
                    docstring: None,
                    attributes: Vec::new(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: self.get_visibility(node, content),
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "imports_statement" {
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Visual Basic: Imports Namespace
        format!("Imports {}", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        let lower = text.to_lowercase();
        if lower.contains("private") {
            Visibility::Private
        } else if lower.contains("protected") {
            Visibility::Protected
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/*Test.vb", "**/*Tests.vb"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_end_body(body_node, content, inner_indent)
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
            // Block types
            "namespace_block",
            // Declaration types
            "field_declaration", "constructor_declaration", "event_declaration",
            "type_declaration", "const_declaration", "enum_member",
            // Statement types
            "statement", "assignment_statement", "compound_assignment_statement",
            "call_statement", "dim_statement", "redim_statement", "re_dim_clause",
            "exit_statement", "continue_statement", "return_statement", "goto_statement",
            "label_statement", "throw_statement", "empty_statement",
            // Control flow
            "try_statement", "catch_block", "finally_block",
            "case_block", "case_else_block", "else_clause", "elseif_clause",
            "with_statement", "with_initializer",
            "using_statement", "sync_lock_statement",
            // Expression types
            "expression", "binary_expression", "unary_expression", "ternary_expression",
            "parenthesized_expression", "lambda_expression", "new_expression",
            // Type-related
            "type", "generic_type", "array_type", "primitive_type",
            "type_parameters", "type_parameter", "type_constraint",
            "type_argument_list", "array_rank_specifier",
            // Clauses
            "as_clause", "inherits_clause", "implements_clause",
            // Modifiers
            "modifier", "modifiers",
            // Event handlers
            "add_handler_block", "remove_handler_block", "raise_event_block",
            // Other
            "identifier", "attribute_block", "option_statements",
            "relational_operator", "lambda_parameter",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "case_clause",
            "while_statement",
            "for_statement",
            "for_each_statement",
            "imports_statement",
            "do_statement",
            "if_statement",
            "select_case_statement",
        ];
        validate_unused_kinds_audit(&VB, documented_unused)
            .expect("Visual Basic unused node kinds audit failed");
    }
}
