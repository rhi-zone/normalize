//! Dart language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Dart language support.
pub struct Dart;

impl Language for Dart {
    fn name(&self) -> &'static str {
        "Dart"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }
    fn grammar_name(&self) -> &'static str {
        "dart"
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_dart_annotations(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "superclass" || child.kind() == "interfaces" {
                let mut ic = child.walk();
                for t in child.children(&mut ic) {
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
            k if k.contains("function") || k.contains("method") => {
                let return_type = node
                    .child_by_field_name("return_type")
                    .map(|t| content[t.byte_range()].to_string());
                let params = node
                    .child_by_field_name("formal_parameters")
                    .or_else(|| node.child_by_field_name("parameters"))
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                if let Some(ret) = return_type {
                    format!("{} {}{}", ret, name, params)
                } else {
                    format!("{}{}", name, params)
                }
            }
            "class_declaration" => {
                let is_abstract = node
                    .parent()
                    .map(|p| content[p.byte_range()].contains("abstract "))
                    .unwrap_or(false);
                if is_abstract {
                    format!("abstract class {}", name)
                } else {
                    format!("class {}", name)
                }
            }
            "enum_declaration" => format!("enum {}", name),
            "mixin_declaration" => format!("mixin {}", name),
            "extension_declaration" => format!("extension {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_specification" && node.kind() != "library_export" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract the import URI
        if let Some(start) = text.find('\'').or_else(|| text.find('"')) {
            // normalize-syntax-allow: rust/unwrap-in-impl - start is the byte offset of an ASCII quote char; byte == char index for ASCII
            let quote = text.chars().nth(start).unwrap();
            let rest = &text[start + 1..];
            if let Some(end) = rest.find(quote) {
                let module = rest[..end].to_string();
                let is_relative = module.starts_with('.') || module.starts_with('/');

                // Check for 'as' alias
                let alias = if text.contains(" as ") {
                    text.split(" as ")
                        .nth(1)
                        .and_then(|s| s.split(';').next())
                        .map(|s| s.trim().to_string())
                } else {
                    None
                };

                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias,
                    is_wildcard: text.contains(" show ") || text.contains(" hide "),
                    is_relative,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Dart: import 'package:name/name.dart'; or import '...' show a, b, c;
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import '{}';", import.module)
        } else {
            format!(
                "import '{}' show {};",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if let Some(name) = self.node_name(node, content) {
            if name.starts_with('_') {
                Visibility::Private
            } else {
                Visibility::Public
            }
        } else {
            Visibility::Public
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/test/**/*.dart", "**/*_test.dart"]
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

/// Extract Dart annotations from child and preceding sibling nodes.
fn extract_dart_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "annotation" {
            let text = content[child.byte_range()].trim().to_string();
            if !text.is_empty() {
                attrs.push(text);
            }
        }
    }
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "annotation" {
            let text = content[sibling.byte_range()].trim().to_string();
            if !text.is_empty() {
                attrs.insert(0, text);
            }
            prev = sibling.prev_sibling();
        } else {
            break;
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
            "additive_expression", "additive_operator", "annotation", "as_operator",
            "assert_statement", "assignable_expression", "assignment_expression",
            "assignment_expression_without_cascade", "await_expression", "binary_operator",
            "bitwise_and_expression", "bitwise_operator", "bitwise_or_expression",
            "bitwise_xor_expression", "cascade_section", "case_builtin",
            "catch_parameters", "class_body", "const_object_expression",
            "constant_constructor_signature", "constructor_invocation",
            "constructor_param", "constructor_signature", "constructor_tearoff",
            "declaration", "dotted_identifier_list", "enum_body", "enum_constant",
            "equality_expression", "equality_operator", "expression_statement",
            "extension_body", "extension_type_declaration", "factory_constructor_signature",
            "finally_clause", "for_element", "for_loop_parts", "formal_parameter",
            "formal_parameter_list", "function_expression_body", "function_type",
            "identifier_dollar_escaped", "identifier_list",
            "if_element", "if_null_expression", "import_or_export", "increment_operator",
            "inferred_type", "initialized_identifier", "initialized_identifier_list",
            "initialized_variable_definition", "initializer_list_entry", "interface",
            "interfaces", "is_operator", "label", "lambda_expression",
            "library_import", "library_name", "local_function_declaration",
            "local_variable_declaration", "logical_and_operator", "logical_or_operator",
            "minus_operator", "mixin_application_class", "multiplicative_expression",
            "multiplicative_operator", "named_parameter_types", "negation_operator",
            "new_expression", "normal_parameter_type", "nullable_type",
            "operator_signature", "optional_formal_parameters", "optional_parameter_types",
            "optional_positional_parameter_types", "parameter_type_list",
            "parenthesized_expression", "pattern_variable_declaration",
            "postfix_expression", "postfix_operator", "prefix_operator", "qualified",
            "record_type", "record_type_field", "record_type_named_field",
            "redirecting_factory_constructor_signature", "relational_expression",
            "relational_operator", "representation_declaration", "rethrow_builtin",
            "scoped_identifier", "shift_expression", "shift_operator", "spread_element",
            "static_final_declaration", "static_final_declaration_list", "superclass",
            "super_formal_parameter", "switch_block", "switch_expression",
            "switch_expression_case", "switch_statement_default", "symbol_literal",
            "throw_expression_without_cascade", "tilde_operator", "type_arguments",
            "type_bound", "type_cast", "type_cast_expression", "type_identifier",
            "type_parameter", "type_parameters", "type_test", "type_test_expression",
            "typed_identifier", "unary_expression", "void_type", "yield_each_statement",
            "yield_statement",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "logical_and_expression",
            "for_statement",
            "do_statement",
            "try_statement",
            "return_statement",
            "continue_statement",
            "catch_clause",
            "conditional_expression",
            "break_statement",
            "switch_statement_case",
            "block",
            "switch_statement",
            "if_statement",
            "throw_expression",
            "rethrow_expression",
            "function_body",
            "function_expression",
            "logical_or_expression",
            "library_export",
            "while_statement",
            "import_specification",
            "method_signature",
            "type_alias",
        ];
        validate_unused_kinds_audit(&Dart, documented_unused)
            .expect("Dart unused node kinds audit failed");
    }
}
