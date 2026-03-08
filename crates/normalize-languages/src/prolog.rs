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
        // clause_term has no field names — children are atom, functional_notation,
        // or operator_notation.
        //
        // For fact/rule:    clause_term → functional_notation { function: atom @name }
        // For :- rule head: clause_term → operator_notation → functional_notation { function: atom }
        // For simple fact:  clause_term → atom @name
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "atom" => return Some(&content[child.byte_range()]),
                "functional_notation" => {
                    if let Some(name_node) = child.child_by_field_name("function") {
                        return Some(&content[name_node.byte_range()]);
                    }
                }
                "operator_notation" => {
                    // Head is the first functional_notation inside operator_notation
                    let mut inner = child.walk();
                    for inner_child in child.children(&mut inner) {
                        if inner_child.kind() == "functional_notation"
                            && let Some(name_node) = inner_child.child_by_field_name("function")
                        {
                            return Some(&content[name_node.byte_range()]);
                        }
                    }
                }
                _ => {}
            }
        }
        None
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
            "binary_operator", "prefix_operator", "prexif_operator",
        ];
        validate_unused_kinds_audit(&Prolog, documented_unused)
            .expect("Prolog unused node kinds audit failed");
    }
}
