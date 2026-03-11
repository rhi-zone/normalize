//! Scala language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_scaladoc(node, content)
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_scala_annotations(node, content)
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "trait_definition" => crate::SymbolKind::Trait,
            _ => tag_kind,
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
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
        crate::ImplementsInfo {
            is_interface: false,
            implements,
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        match node.kind() {
            "function_definition" | "function_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                let ret = node
                    .child_by_field_name("return_type")
                    .map(|r| format!(": {}", &content[r.byte_range()]))
                    .unwrap_or_default();
                format!("def {}{}{}", name, params, ret)
            }
            "class_definition" => format!("class {}", name),
            "object_definition" => format!("object {}", name),
            "trait_definition" => format!("trait {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import pkg.Class or import pkg.{A, B} or import pkg._
        if let Some(rest) = text.strip_prefix("import ") {
            let rest = rest.trim();
            let is_wildcard = rest.ends_with("._") || rest.ends_with(".*");
            let has_selectors = rest.contains('{');

            if has_selectors {
                // import pkg.{A, B, C}
                if let Some(brace) = rest.find('{') {
                    let module = rest[..brace].trim_end_matches('.').to_string();
                    let inner = &rest[brace + 1..];
                    let inner = inner.strip_suffix('}').unwrap_or(inner);
                    let names: Vec<String> = inner
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s != "_")
                        .collect();
                    return vec![Import {
                        module,
                        names,
                        alias: None,
                        is_wildcard: inner.contains('_'),
                        is_relative: false,
                        line,
                    }];
                }
            }

            let module = if is_wildcard {
                rest.strip_suffix("._")
                    .or_else(|| rest.strip_suffix(".*"))
                    .unwrap_or(rest)
                    .to_string()
            } else {
                rest.to_string()
            };

            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard,
                is_relative: false,
                line,
            }];
        }

        Vec::new()
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

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        // Scala uses `access_modifier` child with optional `access_qualifier`.
        // `private` → Private, `protected` → Protected, no modifier → Public.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "access_modifier" {
                let text = &content[child.byte_range()];
                if text.starts_with("private") {
                    return Visibility::Private;
                }
                if text.starts_with("protected") {
                    return Visibility::Protected;
                }
            }
            if child.kind() == "modifiers" {
                let mut mc = child.walk();
                for m in child.children(&mut mc) {
                    if m.kind() == "access_modifier" {
                        let text = &content[m.byte_range()];
                        if text.starts_with("private") {
                            return Visibility::Private;
                        }
                        if text.starts_with("protected") {
                            return Visibility::Protected;
                        }
                    }
                }
            }
        }
        Visibility::Public
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

impl LanguageSymbols for Scala {}

/// Extract a ScalaDoc comment (`/** ... */`) preceding a node.
///
/// Walks backwards through siblings looking for a `block_comment` starting with `/**`.
fn extract_scaladoc(node: &Node, content: &str) -> Option<String> {
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        match sibling.kind() {
            "block_comment" => {
                let text = &content[sibling.byte_range()];
                if text.starts_with("/**") {
                    return Some(clean_block_doc_comment(text));
                }
                return None;
            }
            "annotation" => {
                // Skip annotations between doc comment and declaration
            }
            _ => return None,
        }
        prev = sibling.prev_sibling();
    }
    None
}

/// Clean a `/** ... */` block doc comment into plain text.
fn clean_block_doc_comment(text: &str) -> String {
    let lines: Vec<&str> = text
        .strip_prefix("/**")
        .unwrap_or(text)
        .strip_suffix("*/")
        .unwrap_or(text)
        .lines()
        .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join(" ")
}

/// Extract annotations from a Scala definition node.
///
/// Scala annotations (`@deprecated`, `@tailrec`, etc.) appear as `annotation`
/// children of the definition node.
fn extract_scala_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "annotation" {
            attrs.push(content[child.byte_range()].to_string());
        }
    }
    attrs
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
            "access_modifier",     // used in get_visibility (audit matched by "if" substring)
            "access_qualifier", "arrow_renamed_identifier",
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
            // control flow — not extracted as symbols
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
