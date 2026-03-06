//! Nix language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Nix language support.
pub struct Nix;

impl Language for Nix {
    fn name(&self) -> &'static str {
        "Nix"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["nix"]
    }
    fn grammar_name(&self) -> &'static str {
        "nix"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "apply_expression" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("import ") {
            return Vec::new();
        }

        // Extract path after "import"
        let rest = text.strip_prefix("import ").unwrap_or("").trim();
        let module = rest.split_whitespace().next().unwrap_or(rest).to_string();

        vec![Import {
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: rest.starts_with('.'),
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Nix: import ./path.nix
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("attrpath")
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
            "assert_expression", "binary_expression", "float_expression",
            "formal", "formals", "has_attr_expression", "hpath_expression",
            "identifier", "indented_string_expression", "integer_expression",
            "list_expression", "let_attrset_expression", "parenthesized_expression",
            "path_expression", "select_expression", "spath_expression",
            "string_expression", "unary_expression", "uri_expression",
            "variable_expression",
            // Control flow / application — not definition constructs
            "apply_expression", "if_expression", "with_expression",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "let_expression",
            "attrset_expression",
            "rec_attrset_expression",
            "function_expression",
        ];
        validate_unused_kinds_audit(&Nix, documented_unused)
            .expect("Nix unused node kinds audit failed");
    }
}
