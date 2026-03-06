//! Kotlin language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Kotlin language support.
pub struct Kotlin;

impl Kotlin {
    /// Find the first type_identifier in a delegation_specifier subtree.
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

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_kdoc(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> (bool, Vec<String>) {
        let mut implements = Vec::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "delegation_specifier"
            {
                Self::find_type_identifier(&child, content, &mut implements);
            }
        }
        (false, implements)
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
            "function_declaration" | "function_definition" => {
                let params = node
                    .child_by_field_name("value_parameters")
                    .or_else(|| node.child_by_field_name("parameters"))
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                let return_type = node
                    .child_by_field_name("type")
                    .map(|t| format!(": {}", content[t.byte_range()].trim()))
                    .unwrap_or_default();
                format!("fun {}{}{}", name, params, return_type)
            }
            "class_declaration" => format!("class {}", name),
            "object_declaration" => format!("object {}", name),
            "type_alias" => {
                let target = node
                    .child_by_field_name("type")
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/src/test/**/*.kt",
            "**/Test*.kt",
            "**/*Test.kt",
            "**/*Tests.kt",
        ]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("class_body")
            .or_else(|| node.child_by_field_name("body"))
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
        // Try "name" field first (most declarations)
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        // Try first type_identifier (class/object declarations) or simple_identifier
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && (child.kind() == "type_identifier" || child.kind() == "simple_identifier")
            {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_kotlin_annotations(node, content)
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

/// Extract a KDoc comment (`/** ... */`) preceding a node.
///
/// Walks backwards through siblings looking for a `multiline_comment` starting with `/**`.
fn extract_kdoc(node: &Node, content: &str) -> Option<String> {
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

/// Extract annotations from a Kotlin definition node.
/// Kotlin annotations live inside a `modifiers` child (e.g. `@JvmStatic`, `@Deprecated`).
fn extract_kotlin_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "annotation" {
                    attrs.push(content[mod_child.byte_range()].to_string());
                }
            }
        }
    }
    attrs
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
            "constructor_delegation_call", // this(), super()  // constructor call
            "control_structure_body",  // control body
            "delegation_specifier",    // delegation              // enum value
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
            "primary_constructor",     // primary constructor    // property
            "property_modifier",       // property modifiers
            "reification_modifier",    // reified
            "secondary_constructor",   // secondary constructor       // simple id
            "statements",              // statement list
            "visibility_modifier",     // public, private

            // EXPRESSION
            "additive_expression",     // a + b
            "as_expression",           // x as T         // foo()
            "check_expression",        // is, !is
            "comparison_expression",   // a < b
            "directly_assignable_expression", // assignable
            "equality_expression",     // a == b
            "indexing_expression",     // arr[i]
            "infix_expression",        // a infix b
            "multiplicative_expression", // a * b   // a.b
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
            "type_constraints",        // where clause         // type name
            "type_modifiers",          // type modifiers
            "type_parameter",          // T
            "type_parameter_modifiers",// type param mods
            "type_parameters",         // <T, U>
            "type_projection",         // out T, in T
            "type_projection_modifiers", // projection mods
            "type_test",               // is T               // user-defined type
            "variance_modifier",       // in, out

            // OTHER
            "finally_block",           // finally    // var/val decl
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "if_expression",
            "anonymous_function",
            "when_entry",
            "conjunction_expression",
            "disjunction_expression",
            "while_statement",
            "do_while_statement",
            "enum_class_body",
            "for_statement",
            "import_header",
            "elvis_expression",
            "jump_expression",
            "when_expression",
            "try_expression",
            "lambda_literal",
            "catch_block",
        ];

        validate_unused_kinds_audit(&Kotlin, documented_unused)
            .expect("Kotlin unused node kinds audit failed");
    }
}
