//! ReScript language support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// ReScript language support.
pub struct ReScript;

impl Language for ReScript {
    fn name(&self) -> &'static str {
        "ReScript"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["res", "resi"]
    }
    fn grammar_name(&self) -> &'static str {
        "rescript"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        match node.kind() {
            "let_binding" | "external_declaration" => {
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
            _ => None,
        }
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "module_declaration" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
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

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "type_declaration" {
            return None;
        }

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
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "open_statement" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: true,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // ReScript: open Module
        format!("open {}", import.module)
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

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
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
            // Expression nodes
            "try_expression", "ternary_expression", "while_expression", "for_expression",
            "call_expression", "pipe_expression", "sequence_expression", "await_expression",
            "coercion_expression", "lazy_expression", "assert_expression",
            "parenthesized_expression", "unary_expression", "binary_expression",
            "subscript_expression", "member_expression", "mutation_expression",
            "extension_expression",
            // Type nodes
            "type_identifier", "type_identifier_path", "unit_type", "generic_type",
            "function_type", "polyvar_type", "polymorphic_type", "tuple_type",
            "record_type", "record_type_field", "object_type", "variant_type",
            "abstract_type", "type_arguments", "type_parameters", "type_constraint",
            "type_annotation", "type_binding", "type_spread", "constrain_type",
            "as_aliasing_type", "function_type_parameters",
            // Module nodes
            "parenthesized_module_expression", "module_type_constraint", "module_type_annotation",
            "module_type_of", "constrain_module", "module_identifier", "module_identifier_path",
            "module_pack", "module_unpack", "module_binding",
            // Declaration nodes
            "let_declaration", "exception_declaration", "variant_declaration",
            "polyvar_declaration", "include_statement",
            // JSX
            "jsx_expression", "jsx_identifier", "nested_jsx_identifier",
            // Pattern matching
            "exception_pattern", "polyvar_type_pattern",
            // Identifiers
            "value_identifier", "value_identifier_path", "variant_identifier",
            "nested_variant_identifier", "polyvar_identifier", "property_identifier",
            "extension_identifier", "decorator_identifier",
            // Clauses
            "else_clause", "else_if_clause",
            // Other
            "function", "expression_statement", "formal_parameters",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "if_expression",
            "block",
            "switch_expression",
            "open_statement",
            "switch_match",
        ];
        validate_unused_kinds_audit(&ReScript, documented_unused)
            .expect("ReScript unused node kinds audit failed");
    }
}
