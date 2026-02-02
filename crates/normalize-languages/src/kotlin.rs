//! Kotlin language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Kotlin language support.
pub struct Kotlin;

impl Language for Kotlin {
    fn name(&self) -> &'static str {
        "Kotlin"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }
    fn grammar_name(&self) -> &'static str {
        "kotlin"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_declaration", "enum_class_body"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "anonymous_function",
            "lambda_literal",
        ]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_declaration", "type_alias"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_header"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "object_declaration",
            "function_declaration",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AccessModifier
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if self.get_visibility(node, content) != Visibility::Public {
            return Vec::new();
        }

        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "class_declaration" => SymbolKind::Class,
            "object_declaration" => SymbolKind::Class, // object is a singleton class
            "function_declaration" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "while_statement",
            "do_while_statement",
            "try_expression",
            "catch_block",
            "when_expression",
            "lambda_literal",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "when_expression",
            "try_expression",
            "jump_expression",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "when_entry",
            "catch_block",
            "elvis_expression",
            "conjunction_expression",
            "disjunction_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "when_expression",
            "try_expression",
            "function_declaration",
            "class_declaration",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let params = node
            .child_by_field_name("value_parameters")
            .or_else(|| node.child_by_field_name("parameters"))
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        let return_type = node
            .child_by_field_name("type")
            .map(|t| format!(": {}", content[t.byte_range()].trim()))
            .unwrap_or_default();

        // Check for override modifier
        let is_override = if let Some(mods) = node.child_by_field_name("modifiers") {
            let mut cursor = mods.walk();
            let children: Vec<_> = mods.children(&mut cursor).collect();
            children.iter().any(|child| {
                child.kind() == "member_modifier"
                    && child.child(0).map(|c| c.kind()) == Some("override")
            })
        } else {
            false
        };

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: format!("fun {}{}{}", name, params, return_type),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: is_override,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "object_declaration" => (SymbolKind::Class, "object"),
            _ => (SymbolKind::Class, "class"),
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() == "type_alias" {
            let name = self.node_name(node, content)?;
            let target = node
                .child_by_field_name("type")
                .map(|t| content[t.byte_range()].to_string())
                .unwrap_or_default();
            return Some(Symbol {
                name: name.to_string(),
                kind: SymbolKind::Type,
                signature: format!("typealias {} = {}", name, target),
                docstring: None,
                attributes: Vec::new(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                visibility: self.get_visibility(node, content),
                children: Vec::new(),
                is_interface_impl: false,
                implements: Vec::new(),
            });
        }
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Look for KDoc comment before the node
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            match sibling.kind() {
                "multiline_comment" => {
                    let text = &content[sibling.byte_range()];
                    if text.starts_with("/**") {
                        // Strip /** and */ and leading *
                        let lines: Vec<&str> = text
                            .strip_prefix("/**")
                            .unwrap_or(text)
                            .strip_suffix("*/")
                            .unwrap_or(text)
                            .lines()
                            .map(|l| l.trim().strip_prefix("*").unwrap_or(l).trim())
                            .filter(|l| !l.is_empty())
                            .collect();
                        if !lines.is_empty() {
                            return Some(lines.join(" "));
                        }
                    }
                    return None;
                }
                "line_comment" => {
                    // Skip single-line comments
                }
                _ => return None,
            }
            prev = sibling.prev_sibling();
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_header" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;

        // Get the import identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "user_type" {
                let module = content[child.byte_range()].to_string();
                let is_wildcard = content[node.byte_range()].contains(".*");
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Kotlin: import pkg.Class or import pkg.*
        if import.is_wildcard {
            format!("import {}.*", import.module)
        } else {
            format!("import {}", import.module)
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        self.get_visibility(node, content) == Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("class_body")
            .or_else(|| node.child_by_field_name("body"))
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // Try "name" field first (most declarations)
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        // For type alias, the name might be a simple_identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "simple_identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mods = &content[child.byte_range()];
                if mods.contains("private") {
                    return Visibility::Private;
                }
                if mods.contains("protected") {
                    return Visibility::Protected;
                }
                if mods.contains("internal") {
                    return Visibility::Protected;
                } // internal ≈ protected for our purposes
                if mods.contains("public") {
                    return Visibility::Public;
                }
            }
            // Also check visibility_modifier directly
            if child.kind() == "visibility_modifier" {
                let vis = &content[child.byte_range()];
                if vis == "private" {
                    return Visibility::Private;
                }
                if vis == "protected" {
                    return Visibility::Protected;
                }
                if vis == "internal" {
                    return Visibility::Protected;
                }
                if vis == "public" {
                    return Visibility::Public;
                }
            }
        }
        // Kotlin default is public (unlike Java's package-private)
        Visibility::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the Kotlin grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "annotated_lambda",        // @Ann { }
            "class_body",              // class body
            "class_modifier",          // class modifiers
            "class_parameter",         // class param
            "constructor_delegation_call", // this(), super()
            "constructor_invocation",  // constructor call
            "control_structure_body",  // control body
            "delegation_specifier",    // delegation
            "enum_entry",              // enum value
            "function_body",           // function body
            "function_modifier",       // fun modifiers
            "function_type_parameters",// (T) -> U params
            "function_value_parameters", // fun params
            "identifier",              // too common
            "import_alias",            // import as
            "import_list",             // imports
            "inheritance_modifier",    // open, final
            "interpolated_expression", // ${expr}
            "interpolated_identifier", // $id
            "lambda_parameters",       // lambda params
            "member_modifier",         // member modifiers
            "modifiers",               // modifiers
            "multi_variable_declaration", // val (a, b)
            "parameter_modifier",      // param modifiers
            "parameter_modifiers",     // param modifiers list
            "parameter_with_optional_type", // optional type param
            "platform_modifier",       // expect, actual
            "primary_constructor",     // primary constructor
            "property_declaration",    // property
            "property_modifier",       // property modifiers
            "reification_modifier",    // reified
            "secondary_constructor",   // secondary constructor
            "simple_identifier",       // simple id
            "statements",              // statement list
            "visibility_modifier",     // public, private

            // EXPRESSION
            "additive_expression",     // a + b
            "as_expression",           // x as T
            "call_expression",         // foo()
            "check_expression",        // is, !is
            "comparison_expression",   // a < b
            "directly_assignable_expression", // assignable
            "equality_expression",     // a == b
            "indexing_expression",     // arr[i]
            "infix_expression",        // a infix b
            "multiplicative_expression", // a * b
            "navigation_expression",   // a.b
            "parenthesized_expression",// (expr)
            "postfix_expression",      // x++
            "prefix_expression",       // ++x
            "range_expression",        // 0..10
            "spread_expression",       // *arr
            "super_expression",        // super
            "this_expression",         // this
            "wildcard_import",         // import.*

            // TYPE
            "function_type",           // (T) -> U
            "not_nullable_type",       // T & Any
            "nullable_type",           // T?
            "parenthesized_type",      // (T)
            "parenthesized_user_type", // (UserType)
            "receiver_type",           // T.
            "type_arguments",          // <T, U>
            "type_constraint",         // T : Bound
            "type_constraints",        // where clause
            "type_identifier",         // type name
            "type_modifiers",          // type modifiers
            "type_parameter",          // T
            "type_parameter_modifiers",// type param mods
            "type_parameters",         // <T, U>
            "type_projection",         // out T, in T
            "type_projection_modifiers", // projection mods
            "type_test",               // is T
            "user_type",               // user-defined type
            "variance_modifier",       // in, out

            // OTHER
            "finally_block",           // finally
            "variable_declaration",    // var/val decl
        ];

        validate_unused_kinds_audit(&Kotlin, documented_unused)
            .expect("Kotlin unused node kinds audit failed");
    }
}
