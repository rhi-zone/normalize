//! Swift language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use tree_sitter::Node;

/// Swift language support.
pub struct Swift;

impl Swift {
    /// Find the first type_identifier in an inheritance_specifier subtree.
    fn find_type_identifier(node: &Node, content: &str, out: &mut Vec<String>) {
        let before = out.len();
        if node.kind() == "type_identifier" {
            out.push(content[node.byte_range()].to_string());
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::find_type_identifier(&child, content, out);
            if out.len() > before {
                return;
            }
        }
    }
}

impl Language for Swift {
    fn name(&self) -> &'static str {
        "Swift"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }
    fn grammar_name(&self) -> &'static str {
        "swift"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn refine_kind(
        &self,
        node: &Node,
        content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        // Swift uses class_declaration for class/struct/enum/actor,
        // distinguished by the declaration_kind field.
        if node.kind() == "class_declaration"
            && let Some(kind_node) = node.child_by_field_name("declaration_kind")
        {
            let kind_text = &content[kind_node.byte_range()];
            return match kind_text {
                "struct" => crate::SymbolKind::Struct,
                "enum" => crate::SymbolKind::Enum,
                "class" | "actor" => crate::SymbolKind::Class,
                _ => tag_kind,
            };
        }
        tag_kind
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Swift doc comments use triple-slash `///` lines or `/** */` blocks.
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            match sibling.kind() {
                "comment" => {
                    let text = &content[sibling.byte_range()];
                    if text.starts_with("///") {
                        let line = text.strip_prefix("///").unwrap_or("").trim().to_string();
                        doc_lines.push(line);
                    } else {
                        break;
                    }
                }
                "multiline_comment" => {
                    let text = &content[sibling.byte_range()];
                    if text.starts_with("/**") {
                        let lines: Vec<&str> = text
                            .strip_prefix("/**")
                            .unwrap_or(text)
                            .strip_suffix("*/")
                            .unwrap_or(text)
                            .lines()
                            .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
                            .filter(|l| !l.is_empty())
                            .collect();
                        if lines.is_empty() {
                            return None;
                        }
                        return Some(lines.join(" "));
                    }
                    break;
                }
                "attribute" => {
                    // Skip attributes between doc comment and declaration
                }
                _ => break,
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }
        doc_lines.reverse();
        let joined = doc_lines.join(" ");
        let trimmed = joined.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        if let Some(mods) = node.child_by_field_name("modifiers") {
            let mut cursor = mods.walk();
            for child in mods.children(&mut cursor) {
                if child.kind() == "attribute" {
                    let text = content[child.byte_range()].trim().to_string();
                    if !text.is_empty() {
                        attrs.push(text);
                    }
                }
            }
        }
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute" {
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

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "inheritance_specifier" {
                Self::find_type_identifier(&child, content, &mut implements);
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
            "function_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                let return_type = node
                    .child_by_field_name("return_type")
                    .map(|t| format!(" -> {}", content[t.byte_range()].trim()))
                    .unwrap_or_default();
                format!("func {}{}{}", name, params, return_type)
            }
            "class_declaration" => format!("class {}", name),
            "struct_declaration" => format!("struct {}", name),
            "protocol_declaration" => format!("protocol {}", name),
            "enum_declaration" => format!("enum {}", name),
            "extension_declaration" => format!("extension {}", name),
            "actor_declaration" => format!("actor {}", name),
            "typealias_declaration" => {
                let target = node
                    .child_by_field_name("value")
                    .map(|t| content[t.byte_range()].to_string())
                    .unwrap_or_default();
                format!("typealias {} = {}", name, target)
            }
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

        let line = node.start_position().row + 1;

        // Get the module name
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "simple_identifier" {
                let module = content[child.byte_range()].to_string();
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

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Swift: import Module
        format!("import {}", import.module)
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

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" || child.kind() == "modifier" {
                let mod_text = &content[child.byte_range()];
                if mod_text.contains("private") || mod_text.contains("fileprivate") {
                    return Visibility::Private;
                }
                if mod_text.contains("internal") {
                    return Visibility::Protected;
                }
                if mod_text.contains("public") || mod_text.contains("open") {
                    return Visibility::Public;
                }
            }
        }
        // Swift default is internal
        Visibility::Protected
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
        &["**/*Tests.swift", "**/*Test.swift"]
    }
}

impl LanguageSymbols for Swift {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "as_operator", "associatedtype_declaration", "catch_keyword", "class_body",
            "computed_modify", "constructor_expression", "constructor_suffix", "custom_operator",
            "deinit_declaration", "deprecated_operator_declaration_body", "didset_clause",
            "else", "enum_class_body", "enum_entry", "enum_type_parameters",
            "existential_type", "external_macro_definition", "function_body", "function_modifier",
            "getter_specifier", "identifier", "inheritance_modifier", "inheritance_specifier",
            "interpolated_expression", "key_path_expression", "key_path_string_expression",
            "lambda_function_type", "lambda_function_type_parameters", "lambda_parameter",
            "macro_declaration", "macro_definition", "member_modifier", "metatype", "modifiers",
            "modify_specifier", "mutation_modifier", "opaque_type", "operator_declaration",
            "optional_type", "ownership_modifier", "parameter_modifier", "parameter_modifiers",
            "precedence_group_declaration", "property_behavior_modifier", "property_declaration",
            "property_modifier", "protocol_body", "protocol_composition_type",
            "protocol_function_declaration", "protocol_property_declaration", "self_expression",
            "setter_specifier", "simple_identifier", "statement_label", "statements",
            "super_expression", "switch_entry", "throw_keyword", "throws", "try_operator",
            "tuple_expression", "tuple_type", "tuple_type_item", "type_annotation",
            "type_arguments", "type_constraint", "type_constraints", "type_identifier",
            "type_modifiers", "type_pack_expansion", "type_parameter", "type_parameter_modifiers",
            "type_parameter_pack", "type_parameters", "user_type", "visibility_modifier",
            "where_clause", "willset_clause", "willset_didset_block",
            // EXPRESSION
            "additive_expression", "as_expression", "await_expression", "call_expression",
            "check_expression", "comparison_expression", "conjunction_expression",
            "directly_assignable_expression", "disjunction_expression", "equality_expression",
            "infix_expression", "multiplicative_expression", "navigation_expression",
            "open_end_range_expression", "open_start_range_expression", "postfix_expression",
            "prefix_expression", "range_expression", "selector_expression", "try_expression",
            // TYPE
            "array_type", "dictionary_type", "function_type",
            // covered by tags.scm
            "init_declaration",
            "repeat_while_statement",
            "while_statement",
            "import_declaration",
            "subscript_declaration",
            "lambda_literal",
            "for_statement",
            "if_statement",
            "nil_coalescing_expression",
            "do_statement",
            "ternary_expression",
            "catch_block",
            "control_transfer_statement",
            "switch_statement",
            "guard_statement",
        ];

        validate_unused_kinds_audit(&Swift, documented_unused)
            .expect("Swift unused node kinds audit failed");
    }
}
