//! Idris language support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// Idris language support.
pub struct Idris;

impl Language for Idris {
    fn name(&self) -> &'static str {
        "Idris"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["idr", "lidr"]
    }
    fn grammar_name(&self) -> &'static str {
        "idris"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
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
        // Idris: import Module
        format!("import {}", import.module)
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
        // Idris: data → data_body, record → record_body, interface → interface_body
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if matches!(child.kind(), "data_body" | "record_body" | "interface_body") {
                return Some(child);
            }
        }
        None
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // interface_body starts with `where\n` — skip that first line.
        // data_body and record_body start directly with content.
        match body_node.kind() {
            "interface_body" => {
                crate::body::analyze_keyword_end_body(body_node, content, inner_indent)
            }
            _ => crate::body::analyze_end_body(body_node, content, inner_indent),
        }
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
            "exp_else", "exp_with", "exp_lambda", "exp_lambda_case",
            "exp_list_comprehension", "lambda_exp", "lambda_args",
            // Type-related
            "type_signature", "type_parens", "type_braces", "type_var", "forall",
            // Body nodes
            "parameters_body", "namespace_body", "mutual_body", "data_body",
            "record_body", "interface_body", "implementation_body",
            // Interface and module
            "interface_head", "interface_name", "module",
            // Operators
            "operator", "qualified_operator", "qualified_dot_operators", "dot_operator",
            "ticked_operator", "tuple_operator",
            // Qualified names
            "qualified_loname", "qualified_caname",
            // Other constructs
            "constructor", "statement", "declarations",
            "with", "with_pat", "with_arg",
            // Pragmas
            "pragma_export", "pragma_foreign", "pragma_foreign_impl", "pragma_transform",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "exp_if",
            "exp_case",
            "import",
        ];
        validate_unused_kinds_audit(&Idris, documented_unused)
            .expect("Idris unused node kinds audit failed");
    }
}
