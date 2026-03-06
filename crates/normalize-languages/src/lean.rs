//! Lean language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Lean language support.
pub struct Lean;

impl Language for Lean {
    fn name(&self) -> &'static str {
        "Lean"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["lean"]
    }
    fn grammar_name(&self) -> &'static str {
        "lean"
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

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Lean: import Module or open Module (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!("open {} ({})", import.module, names_to_use.join(", "))
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.contains("private") {
            Visibility::Private
        } else if text.contains("protected") {
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
            // Expression nodes (not declarations)
            "for_in", "binary_expression", "unary_expression",
            "type_ascription", "forall", "subtype", "structure_instance",
            "anonymous_constructor", "lift_method",
            // Parts of declarations
            "constructor", "declaration", "identifier",
            // Module system
            "module", "export",
            // Control flow
            "do_return",
            // Classes
            "class_inductive",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "if_then_else",
            "match_alt",
            "match",
            "import",
        ];
        validate_unused_kinds_audit(&Lean, documented_unused)
            .expect("Lean unused node kinds audit failed");
    }
}
