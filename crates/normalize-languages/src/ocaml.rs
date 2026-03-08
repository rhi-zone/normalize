//! OCaml language support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// OCaml language support.
pub struct OCaml;

impl Language for OCaml {
    fn name(&self) -> &'static str {
        "OCaml"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ml", "mli"]
    }
    fn grammar_name(&self) -> &'static str {
        "ocaml"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_ocamldoc(node, content)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "open_module" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract module name: "open Module.Path"
        if let Some(rest) = text.strip_prefix("open ") {
            let module = rest.trim().to_string();
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: true,
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // OCaml: open Module
        format!("open {}", import.module)
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
        match node.kind() {
            "module_definition" => {
                // module_definition → module_binding → body (structure/functor)
                let mut c = node.walk();
                node.children(&mut c)
                    .find(|n| n.kind() == "module_binding")
                    .and_then(|binding| binding.child_by_field_name("body"))
            }
            _ => node.child_by_field_name("body"),
        }
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // OCaml module bodies: "struct ... end" or "sig ... end" —
        // skip the opening keyword line, strip "end" from the tail
        crate::body::analyze_keyword_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // Try standard field names first
        if let Some(n) = node.child_by_field_name("name") {
            return Some(&content[n.byte_range()]);
        }

        let kind = node.kind();
        let mut cursor = node.walk();

        match kind {
            // value_definition > let_binding > value_name (first)
            "value_definition" => {
                for child in node.children(&mut cursor) {
                    if child.kind() == "let_binding" {
                        let mut inner = child.walk();
                        for c in child.children(&mut inner) {
                            if c.kind() == "value_name" {
                                return Some(&content[c.byte_range()]);
                            }
                        }
                    }
                }
                None
            }
            // module_definition > module_binding > module_name
            "module_definition" => {
                for child in node.children(&mut cursor) {
                    if child.kind() == "module_binding" {
                        let mut inner = child.walk();
                        for c in child.children(&mut inner) {
                            if c.kind() == "module_name" {
                                return Some(&content[c.byte_range()]);
                            }
                        }
                    }
                }
                None
            }
            // module_type_definition > module_type_name (direct child)
            "module_type_definition" => {
                for child in node.children(&mut cursor) {
                    if child.kind() == "module_type_name" {
                        return Some(&content[child.byte_range()]);
                    }
                }
                None
            }
            // type_definition > type_binding > type_constructor (via name: field)
            "type_definition" => {
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_binding"
                        && let Some(n) = child.child_by_field_name("name")
                    {
                        return Some(&content[n.byte_range()]);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// Extract an OCamldoc comment (`(** ... *)`) preceding a definition node.
///
/// OCamldoc comments are parsed as `comment` nodes by tree-sitter-ocaml.
/// We look for a prev sibling `comment` that starts with `(**`.
fn extract_ocamldoc(node: &Node, content: &str) -> Option<String> {
    let sibling = node.prev_sibling()?;
    if sibling.kind() != "comment" {
        return None;
    }
    let text = &content[sibling.byte_range()];
    if text.starts_with("(**") && !text.starts_with("(***") {
        Some(clean_ocamldoc(text))
    } else {
        None
    }
}

/// Clean an OCamldoc comment `(** ... *)` into plain text.
fn clean_ocamldoc(text: &str) -> String {
    let inner = text
        .strip_prefix("(**")
        .unwrap_or(text)
        .strip_suffix("*)")
        .unwrap_or(text);
    let lines: Vec<&str> = inner
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "abstract_type", "add_operator", "aliased_type", "and_operator",
            "application_expression", "array_expression", "array_get_expression",
            "assert_expression", "assign_operator", "bigarray_get_expression",
            "class_application", "class_binding", "class_body_type",
            "class_definition", "class_function", "class_function_type",
            "class_initializer", "class_name", "class_path", "class_type_binding",
            "class_type_definition", "class_type_name", "class_type_path",
            "coercion_expression", "concat_operator", "cons_expression",
            "constrain_module", "constrain_module_type", "constrain_type",
            "constructed_type", "constructor_declaration", "constructor_name",
            "constructor_path", "constructor_pattern", "conversion_specification",
            "do_clause", "else_clause", "exception_definition", "exception_pattern",
            "expression_item", "extended_module_path", "field_declaration",
            "field_expression", "field_get_expression", "for_expression",
            "fun_expression", "function_type", "functor_type", "hash_expression",
            "hash_operator", "hash_type", "include_module", "include_module_type", "infix_expression",
            "indexing_operator", "indexing_operator_path", "inheritance_definition",
            "inheritance_specification", "instance_variable_definition",
            "instance_variable_expression", "instance_variable_specification",
            "instantiated_class", "instantiated_class_type", "labeled_argument_type",
            "labeled_tuple_element_type", "lazy_expression", "let_and_operator",
            "let_class_expression", "let_exception_expression",
            "let_module_expression", "let_open_class_expression",
            "let_open_class_type", "let_open_expression", "let_operator",
            "list_expression", "local_open_expression", "local_open_type",
            "match_operator", "method_definition", "method_invocation",
            "method_name", "method_specification", "method_type", "module_application", "module_parameter", "module_path",
            "module_type_constraint", "module_type_of",
            "module_type_path", "mult_operator", "new_expression", "object_copy_expression",
            "object_expression", "object_type", "or_operator",
            "package_expression", "package_type", "packed_module",
            "parenthesized_class_expression", "parenthesized_expression",
            "parenthesized_module_expression", "parenthesized_module_type",
            "parenthesized_operator", "parenthesized_type", "polymorphic_type",
            "polymorphic_variant_type", "pow_operator", "prefix_expression",
            "prefix_operator", "record_declaration", "record_expression",
            "refutation_case", "rel_operator", "sequence_expression",
            "set_expression", "sign_expression", "sign_operator",
            "string_get_expression", "structure", "tag_specification",
            "then_clause", "tuple_expression", "tuple_type",
            "type_constraint", "type_constructor", "type_constructor_path",
            "type_parameter_constraint", "type_variable", "typed_class_expression",
            "typed_expression", "typed_module_expression", "typed_pattern",
            "value_specification", "variant_declaration", "while_expression",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "match_expression",
            "open_module",
            "let_expression",
            "match_case",
            "function_expression",
            "if_expression",
            "try_expression",
        ];
        validate_unused_kinds_audit(&OCaml, documented_unused)
            .expect("OCaml unused node kinds audit failed");
    }
}
