//! Agda language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Agda language support.
pub struct Agda;

impl Language for Agda {
    fn name(&self) -> &'static str {
        "Agda"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["agda"]
    }
    fn grammar_name(&self) -> &'static str {
        "agda"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        match node.kind() {
            "import" | "open" => {
                let text = &content[node.byte_range()];
                vec![Import {
                    module: text.trim().to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: node.kind() == "open",
                    is_relative: false,
                    line: node.start_position().row + 1,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Agda: open import Module or import Module using (a; b; c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("open import {}", import.module)
        } else {
            format!(
                "import {} using ({})",
                import.module,
                names_to_use.join("; ")
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Agda has no dedicated body field; use the container node itself
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // Agda: module/record body comes after the `where` keyword
        let end = body_node.end_byte();
        let bytes = content.as_bytes();

        let mut content_start = body_node.start_byte();
        let mut c = body_node.walk();
        for child in body_node.children(&mut c) {
            if child.kind() == "where" {
                content_start = child.end_byte();
                if content_start < end && bytes[content_start] == b'\n' {
                    content_start += 1;
                }
                break;
            }
        }

        let is_empty = content[content_start..end].trim().is_empty();
        Some(ContainerBody {
            content_start,
            content_end: end,
            inner_indent: inner_indent.to_string(),
            is_empty,
        })
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "id" || child.kind() == "qid" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }
}

impl LanguageSymbols for Agda {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Function and lambda definitions
            "catchall_pragma", "forall", "lambda",
            "lambda_clause_absurd", "type_signature",
            // Module-related
            "import_directive", "module_application", "module_assignment", "module_macro",
            // Record definitions
            "record_constructor", "record_constructor_instance", "record_declarations_block",
            // Bindings
            "typed_binding", "untyped_binding", "with_expressions",
            // control flow — not extracted as symbols
            "lambda_clause",
            "import",
        ];
        validate_unused_kinds_audit(&Agda, documented_unused)
            .expect("Agda unused node kinds audit failed");
    }
}
