//! D language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// D language support.
pub struct D;

impl D {
    /// Recursively collect type names from a D inheritance clause.
    /// D nests: base_class_list > super_class_or_interface/interfaces/interface >
    /// qualified_identifier > identifier(s)
    fn collect_identifiers(node: &Node, content: &str, out: &mut Vec<String>) {
        if node.kind() == "qualified_identifier" {
            out.push(content[node.byte_range()].to_string());
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_identifiers(&child, content, out);
        }
    }
}

impl Language for D {
    fn name(&self) -> &'static str {
        "D"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["d", "di"]
    }
    fn grammar_name(&self) -> &'static str {
        "d"
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            match sibling.kind() {
                "comment" => {
                    if text.starts_with("///") {
                        let line = text.strip_prefix("///").unwrap_or(text).trim();
                        if !line.is_empty() {
                            doc_lines.push(line.to_string());
                        }
                        prev = sibling.prev_sibling();
                    } else {
                        break;
                    }
                }
                "block_comment" => {
                    if text.starts_with("/**") {
                        let inner = text
                            .strip_prefix("/**")
                            .unwrap_or(text)
                            .strip_suffix("*/")
                            .unwrap_or(text);
                        for line in inner.lines() {
                            let clean = line.trim().strip_prefix('*').unwrap_or(line).trim();
                            if !clean.is_empty() {
                                doc_lines.push(clean.to_string());
                            }
                        }
                    }
                    break;
                }
                "nesting_block_comment" => {
                    if text.starts_with("/++") {
                        let inner = text
                            .strip_prefix("/++")
                            .unwrap_or(text)
                            .strip_suffix("+/")
                            .unwrap_or(text);
                        for line in inner.lines() {
                            let clean = line.trim().strip_prefix('+').unwrap_or(line).trim();
                            if !clean.is_empty() {
                                doc_lines.push(clean.to_string());
                            }
                        }
                    }
                    break;
                }
                _ => break,
            }
        }
        if doc_lines.is_empty() {
            return None;
        }
        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute_specifier" {
                let text = content[child.byte_range()].trim().to_string();
                if !text.is_empty() {
                    attrs.push(text);
                }
            }
        }
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_specifier" {
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

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = self.node_name(node, content).unwrap_or("");
        match node.kind() {
            "module_declaration" => format!("module {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> (bool, Vec<String>) {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_list" {
                D::collect_identifiers(&child, content, &mut implements);
            }
        }
        (false, implements)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: text.contains(':'),
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // D: import module; or import module : a, b, c;
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {};", import.module)
        } else {
            format!("import {} : {};", import.module, names_to_use.join(", "))
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.starts_with("private ") {
            Visibility::Private
        } else if text.starts_with("protected ") {
            Visibility::Protected
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
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
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
            // Expressions
            "add_expression", "and_and_expression", "and_expression", "assign_expression",
            "assert_expression", "cat_expression", "cast_expression", "comma_expression",
            "complement_expression", "conditional_expression", "delete_expression", "equal_expression",
            "expression", "identity_expression", "import_expression", "in_expression",
            "index_expression", "is_expression", "key_expression", "lwr_expression",
            "mixin_expression", "mul_expression", "new_anon_class_expression", "new_expression",
            "or_expression", "or_or_expression", "postfix_expression", "pow_expression",
            "primary_expression", "qualified_identifier", "rel_expression", "shift_expression",
            "slice_expression", "traits_expression", "typeid_expression", "unary_expression",
            "upr_expression", "value_expression", "xor_expression",
            // Statements
            "asm_statement", "break_statement", "case_range_statement", "case_statement",
            "conditional_statement", "continue_statement", "declaration_statement", "default_statement",
            "do_statement", "empty_statement", "expression_statement", "final_switch_statement",
            "foreach_range_statement", "goto_statement", "labeled_statement", "mixin_statement",
            "out_statement", "pragma_statement", "return_statement", "scope_block_statement",
            "scope_guard_statement", "scope_statement_list", "statement_list",
            "statement_list_no_case_no_default", "static_foreach_statement", "synchronized_statement",
            "then_statement", "throw_statement", "try_statement", "with_statement",
            // Declarations
            "anonymous_enum_declaration", "anonymous_enum_member",
            "anonymous_enum_members", "anon_struct_declaration", "anon_union_declaration",
            "auto_func_declaration", "class_template_declaration",
            "conditional_declaration", "debug_specification", "destructor", "empty_declaration",
            "enum_body", "enum_member", "enum_member_attribute", "enum_member_attributes",
            "enum_members", "func_declaration", "interface_template_declaration", "mixin_declaration",
            "module", "shared_static_constructor", "shared_static_destructor", "static_constructor",
            "static_destructor", "static_foreach_declaration", "struct_template_declaration",
            "template_declaration", "template_mixin_declaration", "union_declaration",
            "union_template_declaration", "var_declarations", "version_specification",
            // Foreach-related
            "aggregate_foreach", "foreach", "foreach_aggregate", "foreach_type",
            "foreach_type_attribute", "foreach_type_attributes", "foreach_type_list",
            "range_foreach", "static_foreach",
            // Function-related
            "constructor_args", "constructor_template", "function_attribute_kwd",
            "function_attributes", "function_contracts", "function_literal_body",
            "function_literal_body2", "member_function_attribute", "member_function_attributes",
            "missing_function_body", "out_contract_expression", "in_contract_expression",
            "in_statement", "parameter_with_attributes", "parameter_with_member_attributes",
            "shortened_function_body", "specified_function_body",
            // Template-related
            "template_type_parameter", "template_type_parameter_default",
            "template_type_parameter_specialization", "type_specialization",
            // Type-related
            "aggregate_body", "basic_type", "catch_parameter", "catches", "constructor",
            "else_statement", "enum_base_type", "finally_statement", "fundamental_type",
            "if_condition", "interfaces", "linkage_type", "module_alias_identifier",
            "module_attributes", "module_fully_qualified_name", "module_name", "mixin_type",
            "mixin_qualified_identifier", "storage_class", "storage_classes", "type",
            "type_ctor", "type_ctors", "type_suffix", "type_suffixes", "typeof", "interface",
            // Import-related
            "import", "import_bind", "import_bind_list", "import_bindings", "import_list",
            // ASM-related
            "asm_instruction", "asm_instruction_list", "asm_shift_exp", "asm_type_prefix",
            "gcc_asm_instruction_list", "gcc_asm_statement", "gcc_basic_asm_instruction",
            "gcc_ext_asm_instruction", "gcc_goto_asm_instruction",
            // Misc
            "alt_declarator_identifier", "base_class_list", "base_interface_list",
            "block_comment", "declaration_block", "declarator_identifier_list", "dot_identifier", "nesting_block_comment", "static_if_condition", "struct_initializer",
            "struct_member_initializer", "struct_member_initializers", "super_class_or_interface",
            "traits_arguments", "traits_keyword", "var_declarator_identifier", "vector_base_type",
            "attribute_specifier",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "alias_declaration",
            "auto_declaration",
            "module_declaration",
            "block_statement",
            "import_declaration",
            "while_statement",
            "switch_statement",
            "if_statement",
            "function_literal",
            "for_statement",
            "foreach_statement",
            "catch",
        ];
        validate_unused_kinds_audit(&D, documented_unused)
            .expect("D unused node kinds audit failed");
    }
}
