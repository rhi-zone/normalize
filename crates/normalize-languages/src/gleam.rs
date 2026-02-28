//! Gleam language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Gleam language support.
pub struct Gleam;

impl Language for Gleam {
    fn name(&self) -> &'static str {
        "Gleam"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["gleam"]
    }
    fn grammar_name(&self) -> &'static str {
        "gleam"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["type_definition", "type_alias"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["type_definition", "type_alias"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function", "type_definition", "type_alias", "constant"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport // pub keyword
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let text = &content[node.byte_range()];

        // Only export if marked as pub
        if !text.starts_with("pub ") {
            return Vec::new();
        }

        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "function" => SymbolKind::Function,
            "type_definition" => SymbolKind::Type,
            "type_alias" => SymbolKind::Type,
            "constant" => SymbolKind::Variable,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["function", "anonymous_function"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["case", "if"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["case", "case_clause", "if"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["function", "case", "block"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "function" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);
        let is_public = text.starts_with("pub ");

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if is_public {
                Visibility::Public
            } else {
                Visibility::Private
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);
        let is_public = text.starts_with("pub ");

        let kind = match node.kind() {
            "type_definition" => SymbolKind::Type,
            "type_alias" => SymbolKind::Type,
            _ => return None,
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if is_public {
                Visibility::Public
            } else {
                Visibility::Private
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Gleam uses /// for doc comments
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" && text.starts_with("///") {
                let line = text.strip_prefix("///").unwrap_or(text).trim();
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
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import module/path
        if let Some(rest) = text.strip_prefix("import ") {
            let module = rest.split_whitespace().next().unwrap_or("").to_string();

            if !module.is_empty() {
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Gleam: import module or import module.{a, b, c}
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!("import {}.{{{}}}", import.module, names_to_use.join(", "))
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        content[node.byte_range()].starts_with("pub ")
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self.is_public(node, content) {
            Visibility::Public
        } else {
            Visibility::Private
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
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
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
            // Type-related nodes
            "data_constructor", "data_constructor_argument", "data_constructor_arguments",
            "data_constructors", "external_type", "function_parameter", "function_parameter_types",
            "function_parameters", "function_type", "opacity_modifier", "remote_type_identifier",
            "tuple_type", "type", "type_argument", "type_arguments", "type_hole", "type_identifier",
            "type_name", "type_parameter", "type_parameters", "type_var", "visibility_modifier",
            // Case clause patterns
            "case_clause_guard", "case_clause_pattern", "case_clause_patterns", "case_clauses",
            "case_subjects",
            // Function-related nodes
            "binary_expression", "constructor_name", "external_function", "external_function_body",
            "function_call", "remote_constructor_name",
            // Import-related nodes
            "unqualified_import", "unqualified_imports",
            // Comments and identifiers
            "identifier", "module", "module_comment", "statement_comment",
        ];
        validate_unused_kinds_audit(&Gleam, documented_unused)
            .expect("Gleam unused node kinds audit failed");
    }
}
