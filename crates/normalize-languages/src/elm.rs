//! Elm language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
    simple_function_symbol,
};
use tree_sitter::Node;

/// Elm language support.
pub struct Elm;

impl Language for Elm {
    fn name(&self) -> &'static str {
        "Elm"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["elm"]
    }
    fn grammar_name(&self) -> &'static str {
        "elm"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "module_declaration",
            "type_alias_declaration",
            "type_declaration",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["value_declaration", "function_declaration_left"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["type_alias_declaration", "type_declaration"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_clause"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "value_declaration",
            "type_alias_declaration",
            "type_declaration",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport // module exposing (...)
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "value_declaration" => SymbolKind::Function,
            "type_alias_declaration" => SymbolKind::Type,
            "type_declaration" => SymbolKind::Enum,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["let_in_expr", "anonymous_function_expr"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["if_else_expr", "case_of_expr"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["if_else_expr", "case_of_branch"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["value_declaration", "let_in_expr", "case_of_expr"]
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

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let (kind, keyword) = match node.kind() {
            "module_declaration" => (SymbolKind::Module, "module"),
            "type_alias_declaration" => (SymbolKind::Type, "type alias"),
            "type_declaration" => (SymbolKind::Enum, "type"),
            _ => return None,
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: self.extract_docstring(node, content),
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
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Elm uses {- -} for block comments and -- for line comments
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "block_comment" {
                let inner = text
                    .trim_start_matches("{-|")
                    .trim_start_matches("{-")
                    .trim_end_matches("-}")
                    .trim();
                if !inner.is_empty() {
                    return Some(inner.lines().next().unwrap_or(inner).to_string());
                }
            }
            prev = sibling.prev_sibling();
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_clause" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import Module.Name [as Alias] [exposing (..)]
        if let Some(rest) = text.strip_prefix("import ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(&module) = parts.first() {
                let alias = parts
                    .iter()
                    .position(|&p| p == "as")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());

                return vec![Import {
                    module: module.to_string(),
                    names: Vec::new(),
                    alias,
                    is_wildcard: text.contains("exposing (..)"),
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Elm: import Module or import Module exposing (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {} exposing (..)", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!(
                "import {} exposing ({})",
                import.module,
                names_to_use.join(", ")
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

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
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
            "as_clause", "block_comment", "case", "exposed_operator", "exposed_type",
            "exposed_union_constructors", "field_accessor_function_expr", "field_type",
            "function_call_expr", "import", "infix_declaration", "lower_case_identifier",
            "lower_type_name", "module", "nullary_constructor_argument_pattern",
            "operator", "operator_as_function_expr", "operator_identifier",
            "record_base_identifier", "record_type", "tuple_type", "type",
            "type_annotation", "type_expression", "type_ref", "type_variable",
            "upper_case_identifier", "upper_case_qid",
        ];
        validate_unused_kinds_audit(&Elm, documented_unused)
            .expect("Elm unused node kinds audit failed");
    }
}
