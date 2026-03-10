//! Svelte language support.

use crate::component::extract_embedded_content;
use crate::{ContainerBody, Import, Language, LanguageEmbedded, LanguageSymbols, Visibility};
use tree_sitter::Node;

/// Svelte language support.
pub struct Svelte;

impl Language for Svelte {
    fn name(&self) -> &'static str {
        "Svelte"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["svelte"]
    }
    fn grammar_name(&self) -> &'static str {
        "svelte"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_statement" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract from clause
        if let Some(from_idx) = text.find(" from ") {
            let rest = &text[from_idx + 6..];
            if let Some(start) = rest.find('"').or_else(|| rest.find('\'')) {
                // normalize-syntax-allow: rust/unwrap-in-impl - start is the byte offset of an ASCII quote char; byte == char index for ASCII
                let quote = rest.chars().nth(start).unwrap();
                let inner = &rest[start + 1..];
                if let Some(end) = inner.find(quote) {
                    let module = inner[..end].to_string();
                    return vec![Import {
                        module: module.clone(),
                        names: Vec::new(),
                        alias: None,
                        is_wildcard: text.contains(" * "),
                        is_relative: module.starts_with('.'),
                        line,
                    }];
                }
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Svelte uses JS import syntax
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import '{}';", import.module)
        } else {
            format!(
                "import {{ {} }} from '{}';",
                names_to_use.join(", "),
                import.module
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.contains("export ") {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                name.starts_with("test_")
                    || name.starts_with("Test")
                    || name == "describe"
                    || name == "it"
                    || name == "test"
            }
            crate::SymbolKind::Module => name == "tests" || name == "test" || name == "__tests__",
            _ => false,
        }
    }

    fn as_embedded(&self) -> Option<&dyn LanguageEmbedded> {
        Some(self)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Find the content of script/style elements
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "raw_text")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // raw_text node from script/style element — content after leading newline
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("function"))
            .map(|n| &content[n.byte_range()])
    }
}

impl LanguageSymbols for Svelte {}

impl LanguageEmbedded for Svelte {
    fn embedded_content(&self, node: &Node, content: &str) -> Option<crate::EmbeddedBlock> {
        extract_embedded_content(node, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // Run cross_check_node_kinds to populate
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "await_end", "await_start", "block_end_tag", "block_start_tag",
            "block_tag", "catch_block", "catch_start", "doctype", "else_block",
            "else_if_start", "else_start", "expression", "expression_tag",
            "if_end", "if_start", "key_statement", "snippet_statement", "then_block",
            // Svelte template control flow — not symbol definitions
            "await_statement", "each_statement", "else_if_block", "if_statement",
        ];
        validate_unused_kinds_audit(&Svelte, documented_unused)
            .expect("Svelte unused node kinds audit failed");
    }
}
