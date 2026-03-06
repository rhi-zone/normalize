//! Prolog language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Prolog language support.
pub struct Prolog;

impl Language for Prolog {
    fn name(&self) -> &'static str {
        "Prolog"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pl", "pro", "prolog"]
    }
    fn grammar_name(&self) -> &'static str {
        "prolog"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "directive_term" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if text.contains("use_module(") {
            return vec![Import {
                module: text.trim().to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: false,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Prolog: :- use_module(module) or :- use_module(module, [pred/arity])
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!(":- use_module({}).", import.module)
        } else {
            format!(
                ":- use_module({}, [{}]).",
                import.module,
                names_to_use.join(", ")
            )
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // For clauses, get the predicate name
        let head = if let Some(h) = node.child_by_field_name("head") {
            h
        } else {
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "atom" || child.kind() == "compound_term" {
                    found = Some(child);
                    break;
                }
            }
            found?
        };

        // Get first atom child as the predicate name
        let mut cursor = head.walk();
        for child in head.children(&mut cursor) {
            if child.kind() == "atom" {
                return Some(&content[child.byte_range()]);
            }
        }
        Some(&content[head.byte_range()])
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
            "binary_operator", "functional_notation",
            "operator_notation", "prefix_operator", "prexif_operator",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "clause_term",
        ];
        validate_unused_kinds_audit(&Prolog, documented_unused)
            .expect("Prolog unused node kinds audit failed");
    }
}
