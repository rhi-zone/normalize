//! F# language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// F# language support.
pub struct FSharp;

impl Language for FSharp {
    fn name(&self) -> &'static str {
        "F#"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fs", "fsi", "fsx"]
    }
    fn grammar_name(&self) -> &'static str {
        "fsharp"
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let text = &content[node.byte_range()];
        text.lines().next().unwrap_or(text).trim().to_string()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

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
        // F#: open Namespace
        format!("open {}", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.contains("private ") {
            Visibility::Private
        } else if text.contains("internal ") {
            Visibility::Protected // Using Protected for internal
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
        &["**/*Test.fs", "**/*Tests.fs"]
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
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("identifier"))
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
            "access_modifier", "anon_record_expression", "anon_record_type",
            "anon_type_defn", "array_expression", "atomic_type", "begin_end_expression",
            "block_comment", "block_comment_content", "brace_expression",
            "ce_expression", "class_as_reference", "class_inherits_decl",
            "compound_type", "constrained_type", "declaration_expression",
            "delegate_type_defn", "do_expression", "dot_expression", "elif_expression",
            "enum_type_case", "enum_type_cases", "enum_type_defn",
            "exception_definition", "flexible_type", "format_string",
            "format_string_eval", "format_triple_quoted_string", "fun_expression", "function_expression", "function_type",
            "generic_type", "identifier_pattern", "index_expression", "interface_implementation",
            "interface_type_defn", "list_expression", "list_type", "literal_expression",
            "long_identifier", "long_identifier_or_op",
            "module_abbrev", "mutate_expression", "named_module", "object_expression",
            "op_identifier", "paren_expression", "paren_type", "postfix_type",
            "prefixed_expression", "preproc_else", "preproc_if", "range_expression",
            "sequential_expression", "short_comp_expression", "simple_type",
            "static_type", "trait_member_constraint", "tuple_expression",
            "type_abbrev_defn", "type_argument", "type_argument_constraints",
            "type_argument_defn", "type_arguments", "type_attribute", "type_attributes",
            "type_check_pattern", "type_extension", "type_extension_elements", "typed_expression", "typed_pattern", "typecast_expression",
            "types", "union_type_case", "union_type_cases", "union_type_field",
            "union_type_fields", "value_declaration", "value_declaration_left",
            "with_field_expression",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "union_type_defn",
            "for_expression",
            "application_expression",
            "import_decl",
            "while_expression",
            "match_expression",
            "record_type_defn",
            "infix_expression",
            "if_expression",
            "try_expression",
        ];
        validate_unused_kinds_audit(&FSharp, documented_unused)
            .expect("F# unused node kinds audit failed");
    }
}
