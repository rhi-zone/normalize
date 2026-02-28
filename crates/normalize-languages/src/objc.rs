//! Objective-C language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_interface",
            "class_implementation",
            "protocol_declaration",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["struct_specifier", "enum_specifier", "type_definition"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["preproc_include"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "class_interface",
            "protocol_declaration",
            "method_declaration",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        match node.kind() {
            "class_interface" | "class_implementation" | "protocol_declaration" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            "method_declaration" => {
                if let Some(name) = self.node_name(node, content) {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line: node.start_position().row + 1,
                    }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "class_implementation",
            "method_declaration",
            "function_definition",
            "compound_statement",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "switch_statement",
            "while_statement",
            "for_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "switch_statement",
            "while_statement",
            "for_statement",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "switch_statement",
            "while_statement",
            "for_statement",
            "compound_statement",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        match node.kind() {
            "method_declaration" | "function_definition" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    signature: first_line.trim().to_string(),
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
            _ => None,
        }
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "class_interface" | "class_implementation" | "protocol_declaration" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                // Extract superclass (second identifier after class name) and protocols
                let mut implements = Vec::new();
                let mut found_name = false;
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32) {
                        if child.kind() == "identifier" {
                            if found_name {
                                // Second identifier = superclass
                                implements.push(content[child.byte_range()].to_string());
                            } else {
                                found_name = true;
                            }
                        } else if child.kind() == "parameterized_arguments"
                            || child.kind() == "protocol_qualifiers"
                        {
                            for j in 0..child.child_count() {
                                if let Some(proto) = child.child(j as u32)
                                    && proto.kind() == "type_name"
                                {
                                    for k in 0..proto.child_count() {
                                        if let Some(t) = proto.child(k as u32)
                                            && t.kind() == "type_identifier"
                                        {
                                            implements.push(content[t.byte_range()].to_string());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    signature: first_line.trim().to_string(),
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
            _ => None,
        }
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "struct_specifier" | "enum_specifier" | "type_definition" => {
                let name = self.node_name(node, content)?;
                let text = &content[node.byte_range()];
                let first_line = text.lines().next().unwrap_or(text);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Type,
                    signature: first_line.trim().to_string(),
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
            _ => None,
        }
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
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
            "bitfield_clause", "identifier", "struct_declarator", "gnu_asm_qualifier",
        ];
        validate_unused_kinds_audit(&ObjC, documented_unused)
            .expect("Objective-C unused node kinds audit failed");
    }
}
