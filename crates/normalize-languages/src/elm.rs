//! Elm language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Elm language support.
pub struct Elm;

impl Language for Elm {
    fn name(&self) -> &'static str {
        "Elm"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["elm"]
    }
    fn grammar_name(&self) -> &'static str {
        "elm"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_clause" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import Module.Name [as Alias] [exposing (..)]
        if let Some(rest) = text.strip_prefix("import ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(&module) = parts.first() {
                let alias = parts
                    .iter()
                    .position(|&p| p == "as")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());

                return vec![Import {
                    module: module.to_string(),
                    names: Vec::new(),
                    alias,
                    is_wildcard: text.contains("exposing (..)"),
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Elm: import Module or import Module exposing (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {} exposing (..)", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!(
                "import {} exposing ({})",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let prev = node.prev_sibling()?;

        if prev.kind() != "block_comment" {
            return None;
        }

        let text = &content[prev.byte_range()];
        // Elm doc comments start with {-| and end with -}
        let inner = text.strip_prefix("{-|")?;
        let inner = inner.strip_suffix("-}").unwrap_or(inner).trim().to_string();
        if inner.is_empty() { None } else { Some(inner) }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
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
            "as_clause", "block_comment", "case", "exposed_operator", "exposed_type",
            "exposed_union_constructors", "field_accessor_function_expr", "field_type",
            "function_call_expr", "import", "infix_declaration", "lower_case_identifier",
            "lower_type_name", "module", "nullary_constructor_argument_pattern",
            "operator", "operator_as_function_expr", "operator_identifier",
            "record_base_identifier", "record_type", "tuple_type", "type",
            "type_annotation", "type_expression", "type_ref", "type_variable",
            "upper_case_identifier", "upper_case_qid",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "if_else_expr",
            "import_clause",
            "anonymous_function_expr",
            "module_declaration",
            "case_of_expr",
            "case_of_branch",
        ];
        validate_unused_kinds_audit(&Elm, documented_unused)
            .expect("Elm unused node kinds audit failed");
    }
}
