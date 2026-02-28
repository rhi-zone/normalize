//! Bash language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Bash language support.
pub struct Bash;

impl Language for Bash {
    fn name(&self) -> &'static str {
        "Bash"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash"]
    }
    fn grammar_name(&self) -> &'static str {
        "bash"
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
        &[]
    }
    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }
    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["subshell", "command_substitution"]
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
            "pipeline", // | chains
            "list",     // && and || chains
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "case_statement",
            "function_definition",
            "subshell",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
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

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Bash: source file or . file
        format!("source {}", import.module)
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
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
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
            "binary_expression", "brace_expression", "c_style_for_statement",
            "compound_statement", "declaration_command", "else_clause",
            "heredoc_body", "parenthesized_expression", "postfix_expression",
            "redirected_statement", "ternary_expression", "test_operator",
            "unary_expression",
        ];
        validate_unused_kinds_audit(&Bash, documented_unused)
            .expect("Bash unused node kinds audit failed");
    }
}
