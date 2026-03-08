//! GLSL (OpenGL Shading Language) support.

use crate::{ContainerBody, Language};
use tree_sitter::Node;

/// GLSL language support.
pub struct Glsl;

impl Language for Glsl {
    fn name(&self) -> &'static str {
        "GLSL"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["glsl", "vert", "frag", "geom", "comp", "tesc", "tese"]
    }
    fn grammar_name(&self) -> &'static str {
        "glsl"
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("declarator")
            .and_then(|d| d.child_by_field_name("declarator"))
            .map(|n| &content[n.byte_range()])
            .or_else(|| {
                node.child_by_field_name("name")
                    .map(|n| &content[n.byte_range()])
            })
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
            "abstract_function_declarator", "alignas_qualifier", "alignof_expression",
            "assignment_expression", "attribute_declaration", "attribute_specifier",
            "attributed_statement", "binary_expression", "bitfield_clause", "break_statement",
            "call_expression", "cast_expression", "comma_expression", "compound_literal_expression",
            "continue_statement", "declaration", "declaration_list", "do_statement",
            "else_clause", "enum_specifier", "enumerator", "enumerator_list",
            "expression_statement", "extension_expression", "extension_storage_class",
            "field_declaration", "field_declaration_list", "field_expression", "field_identifier",
            "function_declarator", "generic_expression", "gnu_asm_expression", "gnu_asm_qualifier",
            "goto_statement", "identifier", "labeled_statement", "layout_qualifiers",
            "layout_specification", "linkage_specification", "macro_type_specifier",
            "ms_based_modifier", "ms_call_modifier", "ms_declspec_modifier",
            "ms_pointer_modifier", "ms_restrict_modifier", "ms_signed_ptr_modifier",
            "ms_unaligned_ptr_modifier", "ms_unsigned_ptr_modifier", "offsetof_expression",
            "parameter_declaration", "parenthesized_expression", "pointer_expression",
            "preproc_elif", "preproc_elifdef", "preproc_else", "preproc_function_def",
            "preproc_if", "preproc_ifdef", "primitive_type", "qualifier", "return_statement",
            "seh_except_clause", "seh_finally_clause", "seh_leave_statement", "seh_try_statement",
            "sizeof_expression", "sized_type_specifier", "statement_identifier",
            "storage_class_specifier", "subscript_expression", "type_definition",
            "type_descriptor", "type_identifier", "type_qualifier", "unary_expression",
            "union_specifier", "update_expression",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "switch_statement",
            "if_statement",
            "while_statement",
            "for_statement",
            "conditional_expression",
            "compound_statement",
            "case_statement",
        ];
        validate_unused_kinds_audit(&Glsl, documented_unused)
            .expect("GLSL unused node kinds audit failed");
    }
}
