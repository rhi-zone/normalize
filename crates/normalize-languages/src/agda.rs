//! Agda language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["module", "record"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function", "signature"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["data", "record"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import", "open"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["module", "data", "record", "function"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        match node.kind() {
            "module" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Module,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            "data" | "record" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Type,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            "function" | "signature" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["module", "where"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[] // Agda is purely functional
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["function", "lambda_clause"] // Pattern matching
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["module", "where", "do"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "function" && node.kind() != "signature" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
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

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "module" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Module,
            signature: first_line.trim().to_string(),
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

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "data" | "record" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Type,
                    signature: first_line.trim().to_string(),
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
            _ => None,
        }
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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
        node.child_by_field_name("declarations")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Function and lambda definitions
            "catchall_pragma", "forall", "function_name", "lambda",
            "lambda_clause_absurd", "type_signature",
            // Module-related
            "import_directive", "module_application", "module_assignment", "module_macro",
            "module_name",
            // Record definitions
            "record_constructor", "record_constructor_instance", "record_declarations_block",
            // Bindings
            "typed_binding", "untyped_binding", "with_expressions",
        ];
        validate_unused_kinds_audit(&Agda, documented_unused)
            .expect("Agda unused node kinds audit failed");
    }
}
