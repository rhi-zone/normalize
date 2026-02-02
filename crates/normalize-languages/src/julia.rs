//! Julia language support.

use crate::{
    Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
    simple_function_symbol,
};
use tree_sitter::Node;

/// Julia language support.
pub struct Julia;

impl Language for Julia {
    fn name(&self) -> &'static str {
        "Julia"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jl"]
    }
    fn grammar_name(&self) -> &'static str {
        "julia"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "module_definition",
            "struct_definition",
            "abstract_definition",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "arrow_function_expression",
            "macro_definition",
        ]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[
            "struct_definition",
            "abstract_definition",
            "primitive_definition",
        ]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_statement", "using_statement"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "struct_definition",
            "const_statement",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "function_definition" | "arrow_function_expression" => SymbolKind::Function,
            "macro_definition" => SymbolKind::Function,
            "struct_definition" => SymbolKind::Struct,
            "abstract_definition" => SymbolKind::Interface,
            "module_definition" => SymbolKind::Module,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "let_statement",
            "do_clause",
            "module_definition",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "elseif_clause",
            "ternary_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "module_definition",
            "struct_definition",
            "if_statement",
            "for_statement",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(
            node,
            content,
            name,
            self.extract_docstring(node, content),
        ))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let (kind, keyword) = match node.kind() {
            "module_definition" => (SymbolKind::Module, "module"),
            "struct_definition" => (SymbolKind::Struct, "struct"),
            "abstract_definition" => (SymbolKind::Interface, "abstract type"),
            _ => return None,
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: self.extract_docstring(node, content),
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
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Julia uses """ docstrings before definitions
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "string_literal" && text.starts_with("\"\"\"") {
                let inner = text
                    .trim_start_matches("\"\"\"")
                    .trim_end_matches("\"\"\"")
                    .trim();
                if !inner.is_empty() {
                    return Some(inner.lines().next().unwrap_or(inner).to_string());
                }
            }
            if sibling.kind() == "comment" {
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        let (keyword, is_wildcard) = if text.starts_with("using ") {
            ("using ", true)
        } else if text.starts_with("import ") {
            ("import ", false)
        } else {
            return Vec::new();
        };

        let rest = text.strip_prefix(keyword).unwrap_or("");
        let module = rest
            .split([':', ','])
            .next()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if module.is_empty() {
            return Vec::new();
        }

        vec![Import {
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard,
            is_relative: false,
            line,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Julia: using Module or import Module: a, b, c
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("using {}", import.module)
        } else {
            format!("import {}: {}", import.module, names_to_use.join(", "))
        }
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
            "adjoint_expression", "binary_expression", "block",
            "block_comment", "break_statement", "broadcast_call_expression", "call_expression",
            "catch_clause", "compound_assignment_expression", "compound_statement",
            "comprehension_expression", "continue_statement", "curly_expression", "else_clause",
            "export_statement", "field_expression", "finally_clause", "for_binding", "for_clause",
            "generator", "global_statement", "identifier", "if_clause", "import_alias",
            "import_path", "index_expression", "interpolation_expression",
            "juxtaposition_expression", "local_statement", "macro_identifier",
            "macrocall_expression", "matrix_expression", "operator", "parametrized_type_expression",
            "parenthesized_expression", "public_statement", "quote_expression", "quote_statement",
            "range_expression", "return_statement", "selected_import", "splat_expression",
            "tuple_expression", "type_head", "typed_expression", "unary_expression",
            "unary_typed_expression", "vector_expression", "where_expression",
        ];
        validate_unused_kinds_audit(&Julia, documented_unused)
            .expect("Julia unused node kinds audit failed");
    }
}
