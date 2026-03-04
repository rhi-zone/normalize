//! Scala language support.

use crate::{ContainerBody, Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Scala language support.
pub struct Scala;

impl Language for Scala {
    fn name(&self) -> &'static str {
        "Scala"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }
    fn grammar_name(&self) -> &'static str {
        "scala"
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());
        let ret = node
            .child_by_field_name("return_type")
            .map(|r| format!(": {}", &content[r.byte_range()]))
            .unwrap_or_default();

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            },
            signature: format!("def {}{}{}", name, params, ret),
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
        let name = self.node_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "object_definition" => (SymbolKind::Module, "object"),
            "trait_definition" => (SymbolKind::Trait, "trait"),
            _ => (SymbolKind::Class, "class"),
        };

        // Extract extends/with from extends_clause
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                let mut ec = child.walk();
                for t in child.children(&mut ec) {
                    if t.kind() == "type_identifier" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            }
        }

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements,
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Scala: import pkg.Class or import pkg.{A, B, C}
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {}._", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else if names_to_use.len() == 1 {
            format!("import {}.{}", import.module, names_to_use[0])
        } else {
            format!("import {}.{{{}}}", import.module, names_to_use.join(", "))
        }
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        {
            let has_test_attr = symbol.attributes.iter().any(|a| a.contains("@Test"));
            if has_test_attr {
                return true;
            }
            match symbol.kind {
                crate::SymbolKind::Class => {
                    symbol.name.starts_with("Test") || symbol.name.ends_with("Test")
                }
                _ => false,
            }
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/src/test/**/*.scala",
            "**/*Test.scala",
            "**/*Spec.scala",
            "**/*Suite.scala",
        ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "access_modifier", "access_qualifier", "arrow_renamed_identifier",
            "as_renamed_identifier", "block_comment", "case_block", "case_class_pattern",
            "class_parameter", "class_parameters", "derives_clause", "enum_body",
            "enum_case_definitions", "enum_definition", "enumerator", "enumerators",
            "export_declaration", "extends_clause", "extension_definition", "field_expression",
            "full_enum_case", "identifier", "identifiers", "indented_block", "indented_cases",
            "infix_modifier", "inline_modifier", "instance_expression", "into_modifier",
            "macro_body", "modifiers", "name_and_type", "opaque_modifier", "open_modifier",
            "operator_identifier", "package_clause", "package_identifier", "self_type",
            "simple_enum_case", "template_body", "tracked_modifier", "transparent_modifier",
            "val_declaration", "val_definition", "var_declaration", "var_definition",
            "with_template_body",
            // CLAUSE
            "finally_clause", "type_case_clause",
            // EXPRESSION
            "ascription_expression", "assignment_expression", "call_expression",
            "generic_function", "interpolated_string_expression", "parenthesized_expression",
            "postfix_expression", "prefix_expression", "quote_expression", "splice_expression",
            "tuple_expression",
            // TYPE
            "annotated_type", "applied_constructor_type", "compound_type",
            "contravariant_type_parameter", "covariant_type_parameter", "function_declaration",
            "function_type", "generic_type", "given_definition", "infix_type", "lazy_parameter_type",
            "literal_type", "match_type", "named_tuple_type", "parameter_types",
            "projected_type", "repeated_parameter_type", "singleton_type", "stable_identifier",
            "stable_type_identifier", "structural_type", "tuple_type", "type_arguments", "type_identifier", "type_lambda", "type_parameters", "typed_pattern",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "while_expression",
            "match_expression",
            "catch_clause",
            "import_declaration",
            "return_expression",
            "if_expression",
            "for_expression",
            "throw_expression",
            "block",
            "infix_expression",
            "case_clause",
            "try_expression",
            "do_while_expression",
            "lambda_expression",
        ];

        validate_unused_kinds_audit(&Scala, documented_unused)
            .expect("Scala unused node kinds audit failed");
    }
}
