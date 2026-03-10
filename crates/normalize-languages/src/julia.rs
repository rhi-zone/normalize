//! Julia language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // module_definition has a "name" field
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        // function_definition/macro_definition: name in signature (no named children)
        // struct_definition/abstract_definition: name in type_head
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "signature" || child.kind() == "type_head" {
                let text = &content[child.byte_range()];
                // "add(a, b)" → "add", "Foo <: Bar" → "Foo"
                let end = text
                    .find(|c: char| c == '(' || c == '<' || c == '{' || c.is_whitespace())
                    .unwrap_or(text.len());
                if end > 0 {
                    return Some(&content[child.start_byte()..child.start_byte() + end]);
                }
            }
            if child.kind() == "identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let prev = node.prev_sibling()?;
        if prev.kind() != "string_literal" {
            return None;
        }

        let text = &content[prev.byte_range()];
        if !text.starts_with("\"\"\"") {
            return None;
        }

        // Strip the triple quotes and clean up
        let inner = text
            .strip_prefix("\"\"\"")
            .unwrap_or(text)
            .strip_suffix("\"\"\"")
            .unwrap_or(text);

        let lines: Vec<&str> = inner
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            return None;
        }

        Some(lines.join(" "))
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
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }
}

impl LanguageSymbols for Julia {}

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
            "tuple_expression", "typed_expression", "unary_expression",
            "unary_typed_expression", "vector_expression", "where_expression",
            // covered by tags.scm
            "const_statement",
            "arrow_function_expression",
            "if_statement",
            "using_statement",
            "primitive_definition",
            "for_statement",
            "let_statement",
            "ternary_expression",
            "do_clause",
            "while_statement",
            "try_statement",
            "elseif_clause",
            "import_statement",
        ];
        validate_unused_kinds_audit(&Julia, documented_unused)
            .expect("Julia unused node kinds audit failed");
    }
}
