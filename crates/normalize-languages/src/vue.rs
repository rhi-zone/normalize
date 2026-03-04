//! Vue language support.

use crate::component::extract_embedded_content;
use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
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

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: format!("function {}", name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
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

    fn embedded_content(&self, node: &Node, content: &str) -> Option<crate::EmbeddedBlock> {
        extract_embedded_content(node, content)
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
