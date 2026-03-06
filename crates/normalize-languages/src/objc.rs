//! Objective-C language support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// Objective-C language support.
pub struct ObjC;

impl Language for ObjC {
    fn name(&self) -> &'static str {
        "Objective-C"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["m", "mm"]
    }
    fn grammar_name(&self) -> &'static str {
        "objc"
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let text = &content[node.byte_range()];
        text.lines().next().unwrap_or(text).trim().to_string()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        match node.kind() {
            "preproc_include" => {
                let text = &content[node.byte_range()];
                vec![Import {
                    module: text.trim().to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: text.contains('"'),
                    line: node.start_position().row + 1,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Objective-C: #import <Header.h> or #import "header.h"
        if import.is_relative {
            format!("#import \"{}\"", import.module)
        } else {
            format!("#import <{}>", import.module)
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
        // ObjC has no body field; @interface/@implementation span the whole node
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // Structure: "@interface Foo : Bar\n  methods\n@end"
        // Content starts after the first newline, ends before @end child
        let start = body_node.start_byte();
        let end = body_node.end_byte();
        let bytes = content.as_bytes();

        // Skip past the header line (first \n)
        let mut content_start = start;
        while content_start < end && bytes[content_start] != b'\n' {
            content_start += 1;
        }
        if content_start < end {
            content_start += 1; // skip the \n
        }

        // Find @end child — content ends just before it
        let mut content_end = end;
        let mut c = body_node.walk();
        for child in body_node.children(&mut c) {
            if child.kind() == "@end" {
                content_end = child.start_byte();
                // trim trailing whitespace before @end
                while content_end > content_start
                    && matches!(bytes[content_end - 1], b' ' | b'\t' | b'\n')
                {
                    content_end -= 1;
                }
                break;
            }
        }

        let is_empty = content[content_start..content_end].trim().is_empty();
        Some(ContainerBody {
            content_start,
            content_end,
            inner_indent: inner_indent.to_string(),
            is_empty,
        })
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(n) = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("declarator"))
        {
            return Some(&content[n.byte_range()]);
        }
        // ObjC class_interface/class_implementation: first identifier child is the name
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "identifier"
            {
                return Some(&content[child.byte_range()]);
            }
        }
        None
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
            // Preprocessor
            "preproc_if", "preproc_elif", "preproc_elifdef", "preproc_function_def",
            // Statement types
            "expression_statement", "return_statement", "break_statement", "continue_statement",
            "goto_statement", "case_statement", "labeled_statement", "attributed_statement",
            // Control flow
            "try_statement", "catch_clause", "throw_statement",
            // Expression types
            "binary_expression", "unary_expression", "conditional_expression",
            "call_expression", "subscript_expression", "cast_expression",
            "comma_expression", "assignment_expression", "update_expression",
            "compound_literal_expression", "generic_expression",
            // ObjC specific expressions
            "message_expression", "selector_expression", "encode_expression",
            "at_expression", "available_expression",
            // Declaration types
            "declaration", "declaration_list", "field_declaration_list",
            "property_declaration", "class_declaration", "atomic_declaration",
            "protocol_forward_declaration", "qualified_protocol_interface_declaration",
            "compatibility_alias_declaration",
            // Type system
            "type_name", "type_identifier", "type_qualifier",
            "sized_type_specifier", "array_type_specifier", "macro_type_specifier",
            "typedefed_specifier", "union_specifier", "generic_specifier",
            // Method-related
            "method_definition", "method_identifier", "method_type",
            // Identifiers
            "field_identifier", "statement_identifier",
            // Attributes and specifiers
            "attribute_specifier", "attribute_declaration", "storage_class_specifier",
            "visibility_specification", "property_attributes_declaration",
            "protocol_qualifier", "alignas_qualifier", "alignof_expression",
            "availability_attribute_specifier", "platform",
            // MS extensions
            "ms_restrict_modifier", "ms_unaligned_ptr_modifier", "ms_based_modifier",
            "ms_signed_ptr_modifier", "ms_pointer_modifier", "ms_call_modifier",
            "ms_declspec_modifier", "ms_unsigned_ptr_modifier", "ms_asm_block",
            // GNU extensions
            "gnu_asm_expression", "va_arg_expression", "offsetof_expression",
            // Other
            "function_declarator", "enumerator", "enumerator_list", "else_clause",
            "module_import", "abstract_block_pointer_declarator",
            // Additional expression types
            "extension_expression", "pointer_expression", "parenthesized_expression",
            "sizeof_expression", "range_expression", "field_expression", "block_literal",
            // Declaration and statements
            "implementation_definition", "struct_declaration", "field_declaration",
            "parameter_declaration", "linkage_specification",
            "do_statement", "synchronized_statement", "finally_clause",
            // Type-related
            "typeof_specifier", "type_descriptor", "primitive_type",
            // Preprocessor
            "preproc_else", "preproc_ifdef",
            // Other
            "method_parameter", "block_pointer_declarator", "abstract_function_declarator",
            "bitfield_clause", "struct_declarator", "gnu_asm_qualifier",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "method_declaration",
            "while_statement",
            "for_statement",
            "switch_statement",
            "if_statement",
            "compound_statement",
        ];
        validate_unused_kinds_audit(&ObjC, documented_unused)
            .expect("Objective-C unused node kinds audit failed");
    }
}
