//! Java language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Java language support.
pub struct Java;

impl Language for Java {
    fn name(&self) -> &'static str {
        "Java"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
    fn grammar_name(&self) -> &'static str {
        "java"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "constructor_declaration"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
        ]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_declaration"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "method_declaration",
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
            "interface_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            "method_declaration" | "constructor_declaration" => SymbolKind::Method,
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
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "switch_expression",
            "block",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_expression",
            "try_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "throw_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_label",
            "catch_clause",
            "ternary_expression",
            "binary_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_expression",
            "try_statement",
            "method_declaration",
            "class_declaration",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        // Check for @Override annotation
        let is_override = if let Some(mods) = node.child_by_field_name("modifiers") {
            let mut cursor = mods.walk();
            let children: Vec<_> = mods.children(&mut cursor).collect();
            children.iter().any(|child| {
                child.kind() == "marker_annotation"
                    && child
                        .child(1)
                        .map(|id| &content[id.byte_range()] == "Override")
                        .unwrap_or(false)
            })
        } else {
            false
        };

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Method,
            signature: format!("{}{}", name, params),
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
        let kind = match node.kind() {
            "interface_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            _ => SymbolKind::Class,
        };

        // Extract extends (superclass) and implements (super_interfaces > type_list)
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "superclass" {
                let mut sc = child.walk();
                for t in child.children(&mut sc) {
                    if t.kind() == "type_identifier" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            } else if child.kind() == "super_interfaces" {
                let mut si = child.walk();
                for list in child.children(&mut si) {
                    if list.kind() == "type_list" {
                        let mut tc = list.walk();
                        for t in list.children(&mut tc) {
                            if t.kind() == "type_identifier" {
                                implements.push(content[t.byte_range()].to_string());
                            }
                        }
                    }
                }
            }
        }

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", kind.as_str(), name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements,
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        // Javadoc comments could be extracted but need special handling
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];

        // Extract import path
        let is_static = text.contains("static ");
        let is_wildcard = text.contains(".*");

        // Get the scoped_identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                let module = content[child.byte_range()].to_string();
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: if is_static {
                        Some("static".to_string())
                    } else {
                        None
                    },
                    is_wildcard,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Java: import pkg.Class; or import pkg.*;
        if import.is_wildcard {
            format!("import {}.*;", import.module)
        } else {
            format!("import {};", import.module)
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
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
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
                // public or no modifier = visible in skeleton
                return Visibility::Public;
            }
        }
        // No modifier = package-private, but still visible for skeleton purposes
        Visibility::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the Java grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "block_comment",           // comments
            "class_body",              // class body
            "class_literal",           // Foo.class
            "constructor_body",        // constructor body
            "enum_body",               // enum body
            "enum_body_declarations",  // enum body decls
            "enum_constant",           // enum value
            "field_declaration",       // field decl
            "formal_parameter",        // method param
            "formal_parameters",       // param list
            "identifier",              // too common
            "interface_body",          // interface body
            "modifiers",               // access modifiers
            "scoped_identifier",       // pkg.Class
            "scoped_type_identifier",  // pkg.Type
            "superclass",              // extends
            "super_interfaces",        // implements
            "type_identifier",         // type name

            // CLAUSE
            "catch_formal_parameter",  // catch param
            "catch_type",              // catch type
            "extends_interfaces",      // extends for interfaces
            "finally_clause",          // finally block
            "switch_block",            // switch body
            "switch_block_statement_group", // case group
            "throws",                  // throws clause

            // EXPRESSION
            "array_creation_expression", // new T[]
            "assignment_expression",   // x = y
            "cast_expression",         // (T)x
            "instanceof_expression",   // x instanceof T
            "lambda_expression",       // x -> y
            "method_invocation",       // obj.method()
            "method_reference",        // Class::method
            "object_creation_expression", // new Foo()
            "parenthesized_expression",// (expr)
            "template_expression",     // string template
            "unary_expression",        // -x, !x
            "update_expression",       // x++
            "yield_statement",         // yield x

            // TYPE
            "annotated_type",          // @Ann Type
            "array_type",              // T[]
            "boolean_type",            // boolean
            "floating_point_type",     // float, double
            "generic_type",            // T<U>
            "integral_type",           // int, long
            "type_arguments",          // <T, U>
            "type_bound",              // T extends X
            "type_list",               // T, U, V
            "type_parameter",          // T
            "type_parameters",         // <T, U>
            "type_pattern",            // type pattern
            "void_type",               // void

            // DECLARATION
            "annotation_type_body",    // @interface body
            "annotation_type_declaration", // @interface
            "annotation_type_element_declaration", // @interface element
            "assert_statement",        // assert
            "compact_constructor_declaration", // record constructor
            "constant_declaration",    // const decl
            "explicit_constructor_invocation", // this(), super()
            "expression_statement",    // expr;
            "labeled_statement",       // label: stmt
            "local_variable_declaration", // local var
            "record_declaration",      // record
            "record_pattern_body",     // record pattern

            // MODULE
            "exports_module_directive",// exports
            "module_body",             // module body
            "module_declaration",      // module
            "opens_module_directive",  // opens
            "package_declaration",     // package
            "provides_module_directive", // provides
            "requires_modifier",       // requires modifier
            "requires_module_directive", // requires
            "uses_module_directive",   // uses

            // OTHER
            "resource_specification", // try-with-resources
            "synchronized_statement",  // synchronized
            "try_with_resources_statement", // try-with
        ];

        validate_unused_kinds_audit(&Java, documented_unused)
            .expect("Java unused node kinds audit failed");
    }
}
