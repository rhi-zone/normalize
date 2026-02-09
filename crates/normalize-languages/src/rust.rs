//! Rust language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Rust language support.
pub struct Rust;

impl Language for Rust {
    fn name(&self) -> &'static str {
        "Rust"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
    fn grammar_name(&self) -> &'static str {
        "rust"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["impl_item", "trait_item", "mod_item"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_item"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["struct_item", "enum_item", "type_item", "trait_item"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["use_declaration"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_item", "struct_item", "enum_item", "trait_item"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AccessModifier
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "loop_expression",
            "match_arm",
            "binary_expression", // for && and ||
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "loop_expression",
            "function_item",
            "impl_item",
            "trait_item",
            "mod_item",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        // Additional scope-creating nodes beyond functions and containers
        &[
            "block",
            "for_expression",
            "while_expression",
            "loop_expression",
            "closure_expression",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "loop_expression",
            "return_expression",
            "break_expression",
            "continue_expression",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        // Get visibility modifier
        let mut vis = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                vis = format!("{} ", &content[child.byte_range()]);
                break;
            }
        }

        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        let return_type = node
            .child_by_field_name("return_type")
            .map(|r| format!(" -> {}", &content[r.byte_range()]))
            .unwrap_or_default();

        let signature = format!("{}fn {}{}{}", vis, name, params, return_type);

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            },
            signature,
            docstring: self.extract_docstring(node, content),
            attributes: self.extract_attributes(node, content),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        match node.kind() {
            "impl_item" => {
                let type_node = node.child_by_field_name("type")?;
                let type_name = &content[type_node.byte_range()];

                // Check if this is a trait impl (impl Trait for Type)
                let is_trait_impl = node.child_by_field_name("trait").is_some();

                let (signature, implements) =
                    if let Some(trait_node) = node.child_by_field_name("trait") {
                        let trait_name = &content[trait_node.byte_range()];
                        (
                            format!("impl {} for {}", trait_name, type_name),
                            vec![trait_name.to_string()],
                        )
                    } else {
                        (format!("impl {}", type_name), Vec::new())
                    };

                Some(Symbol {
                    name: type_name.to_string(),
                    kind: SymbolKind::Module, // impl blocks are like modules
                    signature,
                    docstring: None,
                    attributes: self.extract_attributes(node, content),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: Visibility::Public,
                    children: Vec::new(),
                    is_interface_impl: is_trait_impl,
                    implements,
                })
            }
            "trait_item" => {
                let name = self.node_name(node, content)?;
                let vis = self.extract_visibility_prefix(node, content);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Trait,
                    signature: format!("{}trait {}", vis, name),
                    docstring: self.extract_docstring(node, content),
                    attributes: self.extract_attributes(node, content),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: self.get_visibility(node, content),
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                })
            }
            "mod_item" => {
                // Only extract inline mod blocks (with declaration_list), not `mod foo;` declarations
                node.child_by_field_name("body")?;
                let name = self.node_name(node, content)?;
                let vis = self.extract_visibility_prefix(node, content);

                Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Module,
                    signature: format!("{}mod {}", vis, name),
                    docstring: self.extract_docstring(node, content),
                    attributes: self.extract_attributes(node, content),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: self.get_visibility(node, content),
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let vis = self.extract_visibility_prefix(node, content);

        let (kind, keyword) = match node.kind() {
            "struct_item" => (SymbolKind::Struct, "struct"),
            "enum_item" => (SymbolKind::Enum, "enum"),
            "type_item" => (SymbolKind::Type, "type"),
            "trait_item" => (SymbolKind::Trait, "trait"),
            _ => return None,
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{}{} {}", vis, keyword, name),
            docstring: self.extract_docstring(node, content),
            attributes: self.extract_attributes(node, content),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Look for doc comments in the attributes child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attributes" {
                let mut doc_lines = Vec::new();
                let mut attr_cursor = child.walk();
                for attr_child in child.children(&mut attr_cursor) {
                    if attr_child.kind() == "line_outer_doc_comment" {
                        let text = &content[attr_child.byte_range()];
                        let doc = text.trim_start_matches("///").trim();
                        if !doc.is_empty() {
                            doc_lines.push(doc.to_string());
                        }
                    }
                }
                if !doc_lines.is_empty() {
                    return Some(doc_lines.join("\n"));
                }
            }
        }
        None
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();

        // Check for attributes child (e.g., #[test], #[cfg(test)])
        if let Some(attr_node) = node.child_by_field_name("attributes") {
            let mut cursor = attr_node.walk();
            for child in attr_node.children(&mut cursor) {
                if child.kind() == "attribute_item" {
                    attrs.push(content[child.byte_range()].to_string());
                }
            }
        }

        // Also check preceding siblings for outer attributes
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_item" {
                // Insert at beginning to maintain order
                attrs.insert(0, content[sibling.byte_range()].to_string());
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        attrs
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "use_declaration" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];
        let module = text.trim_start_matches("use ").trim_end_matches(';').trim();

        // Check for braced imports: use foo::{bar, baz}
        let mut names = Vec::new();
        let is_relative = module.starts_with("crate")
            || module.starts_with("self")
            || module.starts_with("super");

        if let Some(brace_start) = module.find('{') {
            let prefix = module[..brace_start].trim_end_matches("::");
            if let Some(brace_end) = module.find('}') {
                let items = &module[brace_start + 1..brace_end];
                for item in items.split(',') {
                    let trimmed = item.trim();
                    if !trimmed.is_empty() {
                        names.push(trimmed.to_string());
                    }
                }
            }
            vec![Import {
                module: prefix.to_string(),
                names,
                alias: None,
                is_wildcard: false,
                is_relative,
                line,
            }]
        } else {
            // Simple import: use foo::bar or use foo::bar as baz
            let (module_part, alias) = if let Some(as_pos) = module.find(" as ") {
                (&module[..as_pos], Some(module[as_pos + 4..].to_string()))
            } else {
                (module, None)
            };

            vec![Import {
                module: module_part.to_string(),
                names: Vec::new(),
                alias,
                is_wildcard: module_part.ends_with("::*"),
                is_relative,
                line,
            }]
        }
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());

        if import.is_wildcard {
            // Module already contains ::* from parsing
            format!("use {};", import.module)
        } else if names_to_use.is_empty() {
            format!("use {};", import.module)
        } else if names_to_use.len() == 1 {
            format!("use {}::{};", import.module, names_to_use[0])
        } else {
            format!("use {}::{{{}}};", import.module, names_to_use.join(", "))
        }
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let line = node.start_position().row + 1;

        // Only export pub items
        if !self.is_public(node, content) {
            return Vec::new();
        }

        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "function_item" => SymbolKind::Function,
            "struct_item" => SymbolKind::Struct,
            "enum_item" => SymbolKind::Enum,
            "trait_item" => SymbolKind::Trait,
            _ => return Vec::new(),
        };

        vec![Export { name, kind, line }]
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let vis = &content[child.byte_range()];
                return vis.starts_with("pub");
            }
        }
        false
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let vis = &content[child.byte_range()];
                if vis == "pub" {
                    return Visibility::Public;
                } else if vis.starts_with("pub(crate)") {
                    return Visibility::Internal;
                } else if vis.starts_with("pub(super)") || vis.starts_with("pub(in") {
                    return Visibility::Protected;
                }
            }
        }
        Visibility::Private
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let in_attrs = symbol
            .attributes
            .iter()
            .any(|a| a.contains("#[test]") || a.contains("#[cfg(test)]"));
        let in_sig =
            symbol.signature.contains("#[test]") || symbol.signature.contains("#[cfg(test)]");
        if in_attrs || in_sig {
            return true;
        }
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                symbol.name.starts_with("test_")
            }
            crate::SymbolKind::Module => symbol.name == "tests",
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
        // Rust doesn't have body docstrings, only outer doc comments
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
    }
}

impl Rust {
    fn extract_visibility_prefix(&self, node: &Node, content: &str) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                return format!("{} ", &content[child.byte_range()]);
            }
        }
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the Rust grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        // Categories:
        // - STRUCTURAL: Internal/wrapper nodes
        // - CLAUSE: Sub-parts of larger constructs
        // - EXPRESSION: Expressions (we track statements/definitions)
        // - TYPE: Type-related nodes
        // - MODIFIER: Visibility/async/unsafe modifiers
        // - PATTERN: Pattern matching internals
        // - MACRO: Macro-related nodes
        // - TODO: Potentially useful

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "block_comment",           // comments
            "declaration_list",        // extern block contents
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_expression",        // foo.bar
            "field_identifier",        // field name
            "identifier",              // too common
            "lifetime",                // 'a
            "lifetime_parameter",      // <'a>
            "ordered_field_declaration_list", // tuple struct fields
            "scoped_identifier",       // path::to::thing
            "scoped_type_identifier",  // path::to::Type
            "shorthand_field_identifier", // struct init shorthand
            "type_identifier",         // type names
            "visibility_modifier",     // pub, pub(crate)

            // CLAUSE
            "else_clause",             // part of if
            "enum_variant",            // enum variant
            "enum_variant_list",       // enum body
            "match_block",             // match body
            "match_pattern",           // match arm pattern
            "trait_bounds",            // T: Foo + Bar
            "where_clause",            // where T: Foo

            // EXPRESSION
            "array_expression",        // [1, 2, 3]
            "assignment_expression",   // x = y
            "async_block",             // async { }
            "await_expression",        // foo.await
            "call_expression",         // foo()
            "generic_function",        // foo::<T>()
            "index_expression",        // arr[i]
            "parenthesized_expression",// (expr)
            "range_expression",        // 0..10
            "reference_expression",    // &x
            "struct_expression",       // Foo { x: 1 }
            "try_expression",          // foo?
            "tuple_expression",        // (a, b)
            "type_cast_expression",    // x as T
            "unary_expression",        // -x, !x
            "unit_expression",         // ()
            "yield_expression",        // yield x

            // TYPE
            "abstract_type",           // impl Trait
            "array_type",              // [T; N]
            "bounded_type",            // T: Foo
            "bracketed_type",          // <T>
            "dynamic_type",            // dyn Trait
            "function_type",           // fn(T) -> U
            "generic_type",            // Vec<T>
            "generic_type_with_turbofish", // Vec::<T>
            "higher_ranked_trait_bound", // for<'a>
            "never_type",              // !
            "pointer_type",            // *const T
            "primitive_type",          // i32, bool
            "qualified_type",          // <T as Trait>::Item
            "reference_type",          // &T
            "removed_trait_bound",     // ?Sized
            "tuple_type",              // (A, B)
            "type_arguments",          // <T, U>
            "type_binding",            // Item = T
            "type_parameter",          // T
            "type_parameters",         // <T, U>
            "unit_type",               // ()
            "unsafe_bound_type",       // unsafe trait bound

            // MODIFIER
            "block_outer_doc_comment", // //!
            "extern_modifier",         // extern "C"
            "function_modifiers",      // async, const, unsafe
            "mutable_specifier",       // mut

            // PATTERN
            "struct_pattern",          // Foo { x, y }
            "tuple_struct_pattern",    // Foo(x, y)

            // MACRO
            "fragment_specifier",      // $x:expr
            "macro_arguments_declaration", // macro args
            "macro_body_v2",           // macro body
            "macro_definition",        // macro_rules!
            "macro_definition_v2",     // macro 2.0

            // OTHER
            "block_expression_with_attribute", // #[attr] { }
            "const_block",             // const { }
            "expression_statement",    // expr;
            "expression_with_attribute", // #[attr] expr
            "extern_crate_declaration",// extern crate
            "foreign_mod_item",        // extern block item
            "function_signature_item", // fn signature in trait
            "gen_block",               // gen { }
            "let_declaration",         // let x = y
            "try_block",               // try { }
            "unsafe_block",            // unsafe { }
            "use_as_clause",           // use foo as bar
            "empty_statement",         // ;
        ];

        validate_unused_kinds_audit(&Rust, documented_unused)
            .expect("Rust unused node kinds audit failed");
    }
}
