//! Gleam language support.

use crate::{ContainerBody, Import, Language, Visibility};
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_gleam_doc_comment(node, content)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if content[node.byte_range()].starts_with("pub ") {
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }
}

/// Extract Gleam doc comments (`///`) preceding a declaration.
/// The Gleam tree-sitter grammar may parse these as `statement_comment` or `comment` nodes.
fn extract_gleam_doc_comment(node: &Node, content: &str) -> Option<String> {
    let mut doc_lines: Vec<String> = Vec::new();
    let mut prev = node.prev_sibling();

    while let Some(sibling) = prev {
        let kind = sibling.kind();
        if kind == "comment" || kind == "statement_comment" {
            let text = &content[sibling.byte_range()];
            // Doc comments start with ///
            if let Some(line) = text.strip_prefix("///") {
                let line = line.strip_prefix(' ').unwrap_or(line);
                doc_lines.push(line.to_string());
            } else {
                break;
            }
        } else {
            break;
        }
        prev = sibling.prev_sibling();
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    let joined = doc_lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
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
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "block",
            "import",
            "anonymous_function",
            "case_clause",
            "case",
        ];
        validate_unused_kinds_audit(&Gleam, documented_unused)
            .expect("Gleam unused node kinds audit failed");
    }
}
