//! Vue language support.

use crate::component::extract_embedded_content;
use crate::{ContainerBody, Import, Language, LanguageEmbedded, LanguageSymbols};
use tree_sitter::Node;

/// Vue language support.
pub struct Vue;

impl Language for Vue {
    fn name(&self) -> &'static str {
        "Vue"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vue"]
    }
    fn grammar_name(&self) -> &'static str {
        "vue"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Vue uses JS import syntax
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

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        {
            let name = symbol.name.as_str();
            match symbol.kind {
                crate::SymbolKind::Function | crate::SymbolKind::Method => {
                    name.starts_with("test_")
                        || name.starts_with("Test")
                        || name == "describe"
                        || name == "it"
                        || name == "test"
                }
                crate::SymbolKind::Module => {
                    name == "tests" || name == "test" || name == "__tests__"
                }
                _ => false,
            }
        }
    }

    fn as_embedded(&self) -> Option<&dyn LanguageEmbedded> {
        Some(self)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Vue script/style/template elements contain a raw_text child
        let mut c = node.walk();
        node.children(&mut c)
            .find(|&child| child.kind() == "raw_text")
    }
    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // raw_text node from script/style/template element — content after leading newline
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }
}

impl LanguageSymbols for Vue {}

impl LanguageEmbedded for Vue {
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
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "directive_modifier", "directive_modifiers", "doctype",
        ];

        validate_unused_kinds_audit(&Vue, documented_unused)
            .expect("Vue unused node kinds audit failed");
    }
}
