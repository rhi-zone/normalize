//! Go language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Go language support.
pub struct Go;

impl Language for Go {
    fn name(&self) -> &'static str {
        "Go"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }
    fn grammar_name(&self) -> &'static str {
        "go"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[] // Go types don't have children in the tree-sitter sense
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "method_declaration"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["type_spec"] // The actual type is in type_spec, not type_declaration
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_declaration"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "method_declaration",
            "type_spec",
            "const_spec",
            "var_spec",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "if_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "block",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "goto_statement",
            "defer_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "expression_case",
            "type_case",
            "communication_case",
            "binary_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "function_declaration",
            "method_declaration",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            },
            signature: format!("func {}{}", name, params),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                Visibility::Public
            } else {
                Visibility::Private
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None // Go types are extracted via extract_type
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        // Go type_spec: name field + type field (struct_type, interface_type, etc.)
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        let type_node = node.child_by_field_name("type");
        let type_kind = type_node.map(|t| t.kind()).unwrap_or("");

        let kind = match type_kind {
            "struct_type" => SymbolKind::Struct,
            "interface_type" => SymbolKind::Interface,
            _ => SymbolKind::Type,
        };

        Some(Symbol {
            name: name.clone(),
            kind,
            signature: format!("type {}", name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                Visibility::Public
            } else {
                Visibility::Private
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let mut imports = Vec::new();
        let line = node.start_position().row + 1;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "import_spec" => {
                    // import "path" or import alias "path"
                    if let Some(imp) = Self::parse_import_spec(&child, content, line) {
                        imports.push(imp);
                    }
                }
                "import_spec_list" => {
                    // Grouped imports
                    let mut list_cursor = child.walk();
                    for spec in child.children(&mut list_cursor) {
                        if spec.kind() == "import_spec"
                            && let Some(imp) = Self::parse_import_spec(&spec, content, line)
                        {
                            imports.push(imp);
                        }
                    }
                }
                _ => {}
            }
        }

        imports
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Go: import "pkg" or import alias "pkg"
        if let Some(ref alias) = import.alias {
            format!("import {} \"{}\"", alias, import.module)
        } else {
            format!("import \"{}\"", import.module)
        }
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        // Go exports are determined by uppercase first letter
        let name = match self.node_name(node, content) {
            Some(n) if n.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) => n,
            _ => return Vec::new(),
        };

        let line = node.start_position().row + 1;
        let kind = match node.kind() {
            "function_declaration" => SymbolKind::Function,
            "method_declaration" => SymbolKind::Method,
            "type_spec" => SymbolKind::Type,
            "const_spec" => SymbolKind::Constant,
            "var_spec" => SymbolKind::Variable,
            _ => return Vec::new(),
        };

        vec![Export {
            name: name.to_string(),
            kind,
            line,
        }]
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        self.node_name(node, content)
            .and_then(|n| n.chars().next())
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self.is_public(node, content) {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        match symbol.kind {
            crate::SymbolKind::Function => {
                let name = symbol.name.as_str();
                name.starts_with("Test")
                    || name.starts_with("Benchmark")
                    || name.starts_with("Example")
            }
            _ => false,
        }
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        // Go doc comments could be extracted but need special handling
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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
}

impl Go {
    fn parse_import_spec(node: &Node, content: &str, line: usize) -> Option<Import> {
        let mut path = String::new();
        let mut alias = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "interpreted_string_literal" => {
                    let text = &content[child.byte_range()];
                    path = text.trim_matches('"').to_string();
                }
                "package_identifier" | "blank_identifier" | "dot" => {
                    alias = Some(content[child.byte_range()].to_string());
                }
                _ => {}
            }
        }

        if path.is_empty() {
            return None;
        }

        let is_wildcard = alias.as_deref() == Some(".");
        Some(Import {
            module: path,
            names: Vec::new(),
            alias,
            is_wildcard,
            is_relative: false, // Go doesn't have relative imports in the traditional sense
            line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents node kinds that exist in the Go grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        use crate::validate_unused_kinds_audit;

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "blank_identifier",        // _
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_identifier",        // field name
            "identifier",              // too common
            "package_clause",          // package foo
            "package_identifier",      // package name
            "parameter_declaration",   // func param
            "statement_list",          // block contents
            "variadic_parameter_declaration", // ...T

            // CLAUSE
            "default_case",            // default:
            "for_clause",              // for init; cond; post
            "import_spec",             // import spec
            "import_spec_list",        // import block
            "method_elem",             // interface method
            "range_clause",            // for range

            // EXPRESSION
            "call_expression",         // foo()
            "index_expression",        // arr[i]
            "parenthesized_expression",// (expr)
            "selector_expression",     // foo.bar
            "slice_expression",        // arr[1:3]
            "type_assertion_expression", // x.(T)
            "type_conversion_expression", // T(x)
            "type_instantiation_expression", // generic instantiation
            "unary_expression",        // -x, !x

            // TYPE
            "array_type",              // [N]T
            "channel_type",            // chan T
            "implicit_length_array_type", // [...]T
            "function_type",           // func(T) U
            "generic_type",            // T[U]
            "interface_type",          // interface{}
            "map_type",                // map[K]V
            "negated_type",            // ~T
            "parenthesized_type",      // (T)
            "pointer_type",            // *T
            "qualified_type",          // pkg.Type
            "slice_type",              // []T
            "struct_type",             // struct{}
            "type_arguments",          // [T, U]
            "type_constraint",         // T constraint
            "type_elem",               // type element
            "type_identifier",         // type name
            "type_parameter_declaration", // [T any]
            "type_parameter_list",     // type params

            // DECLARATION
            "assignment_statement",    // x = y
            "const_declaration",       // const x = 1
            "dec_statement",           // x--
            "expression_list",         // a, b, c
            "expression_statement",    // expr
            "inc_statement",           // x++
            "short_var_declaration",   // x := y
            "type_alias",              // type X = Y
            "type_declaration",        // type X struct{}
            "var_declaration",         // var x int

            // CONTROL FLOW DETAILS
            "empty_statement",         // ;
            "fallthrough_statement",   // fallthrough
            "go_statement",            // go foo()
            "labeled_statement",       // label:
            "receive_statement",       // <-ch
            "send_statement",          // ch <- x
        ];

        validate_unused_kinds_audit(&Go, documented_unused)
            .expect("Go unused node kinds audit failed");
    }
}
