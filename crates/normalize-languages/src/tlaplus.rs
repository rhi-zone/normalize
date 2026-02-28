//! TLA+ specification language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// TLA+ language support.
pub struct TlaPlus;

impl Language for TlaPlus {
    fn name(&self) -> &'static str {
        "TLA+"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tla"]
    }
    fn grammar_name(&self) -> &'static str {
        "tlaplus"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["module"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["operator_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["extends"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["module", "operator_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let kind = match node.kind() {
            "module" => SymbolKind::Module,
            "operator_definition" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        if let Some(name) = self.node_name(node, content) {
            return vec![Export {
                name: name.to_string(),
                kind,
                line: node.start_position().row + 1,
            }];
        }
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["module", "operator_definition"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["if_then_else"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["if_then_else", "case"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["module"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "operator_definition" {
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

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "extends" {
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // TLA+: EXTENDS ModuleName
        format!("EXTENDS {}", import.module)
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
            // Declarations and definitions
            "constant_declaration", "variable_declaration", "operator_declaration",
            "local_definition", "function_definition", "module_definition",
            "recursive_declaration", "operator_args",
            // Proof-related
            "definition_proof_step", "case_proof_step", "use_body", "use_body_def",
            "use_body_expr", "statement_level",
            // Quantifiers
            "forall", "quantifier_bound", "tuple_of_identifiers",
            "unbounded_quantification", "bounded_quantification", "temporal_forall",
            // Case expressions
            "case_arm", "case_arrow", "case_box",
            // Function-related
            "function_literal", "function_evaluation", "set_of_functions",
            // Except
            "except", "except_update", "except_update_specifier",
            "except_update_fn_appl", "except_update_record_field",
            // PlusCal
            "pcal_algorithm_body", "pcal_definitions", "pcal_if", "pcal_end_if",
            "pcal_while", "pcal_end_while", "pcal_with", "pcal_end_with",
            "pcal_await", "pcal_return",
            // Identifiers and references
            "identifier", "identifier_ref", "module_ref",
            // Comments
            "block_comment", "block_comment_text",
            // Other
            "lambda", "iff", "format", "subexpression",
        ];
        validate_unused_kinds_audit(&TlaPlus, documented_unused)
            .expect("TLA+ unused node kinds audit failed");
    }
}
