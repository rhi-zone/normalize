//! Zsh language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
    simple_function_symbol,
};
use tree_sitter::Node;

/// Zsh language support.
pub struct Zsh;

impl Language for Zsh {
    fn name(&self) -> &'static str {
        "Zsh"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zsh", "zshrc", "zshenv", "zprofile"]
    }
    fn grammar_name(&self) -> &'static str {
        "zsh"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["command"] // source, .
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "function_definition" {
            return Vec::new();
        }

        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        vec![Export {
            name,
            kind: SymbolKind::Function,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "subshell", "command_substitution"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "case_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "case_statement",
            "case_item",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "if_statement",
            "for_statement",
            "while_statement",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(
            node,
            content,
            name,
            self.extract_docstring(node, content),
        ))
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" && text.starts_with('#') {
                let line = text.strip_prefix('#').unwrap_or(text).trim();
                doc_lines.push(line.to_string());
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // source file or . file
        let module = text
            .strip_prefix("source ")
            .or_else(|| text.strip_prefix(". "))
            .map(|rest| rest.trim().to_string());

        if let Some(module) = module {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: true,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Zsh: source file or . file
        format!("source {}", import.module)
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
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
            "else_clause",
        ];
        validate_unused_kinds_audit(&Zsh, documented_unused)
            .expect("Zsh unused node kinds audit failed");
    }
}
