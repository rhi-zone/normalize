//! Groovy language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Groovy language support.
pub struct Groovy;

impl Language for Groovy {
    fn name(&self) -> &'static str {
        "Groovy"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["groovy", "gradle", "gvy", "gy", "gsh"]
    }
    fn grammar_name(&self) -> &'static str {
        "groovy"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["class_definition"] // Groovy grammar only has class_definition
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "closure"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["class_definition"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["groovy_import"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["class_definition", "function_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AccessModifier // public, private, protected
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "class_definition" => SymbolKind::Class,
            "function_definition" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["class_definition", "function_definition", "closure"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_loop",
            "for_in_loop",
            "while_loop",
            "switch_statement",
            "try_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_loop",
            "for_in_loop",
            "while_loop",
            "switch_statement",
            "case",
            "ternary_op",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "class_definition",
            "function_definition",
            "if_statement",
            "for_loop",
            "closure",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let kind = match node.kind() {
            "class_definition" => SymbolKind::Class,
            _ => return None,
        };

        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
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
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Groovy uses /** */ for Javadoc-style comments
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" {
                if text.starts_with("/**") {
                    let inner = text.trim_start_matches("/**").trim_end_matches("*/").trim();
                    if !inner.is_empty() {
                        // Get first non-empty line, strip leading *
                        for line in inner.lines() {
                            let line = line.trim().trim_start_matches('*').trim();
                            if !line.is_empty() && !line.starts_with('@') {
                                return Some(line.to_string());
                            }
                        }
                    }
                }
            }
            prev = sibling.prev_sibling();
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "groovy_import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import foo.bar.Baz or import foo.bar.*
        if let Some(rest) = text.strip_prefix("import ") {
            let rest = rest.strip_prefix("static ").unwrap_or(rest);
            let module = rest.trim().trim_end_matches(';').to_string();
            let is_wildcard = module.ends_with(".*");

            return vec![Import {
                module: module.trim_end_matches(".*").to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard,
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Groovy: import pkg.Class or import pkg.*
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {}.*", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else if names_to_use.len() == 1 {
            format!("import {}.{}", import.module, names_to_use[0])
        } else {
            // Groovy doesn't have multi-import syntax, so format as module
            format!("import {}", import.module)
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        let text = &content[node.byte_range()];
        !text.starts_with("private") && !text.starts_with("protected")
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.starts_with("private") {
            Visibility::Private
        } else if text.starts_with("protected") {
            Visibility::Protected
        } else {
            Visibility::Public
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
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
            "access_modifier", "array_type", "builtintype", "declaration",
            "do_while_loop", "dotted_identifier", "for_parameters",
            "function_call", "function_declaration", "groovy_doc_throws",
            "identifier", "juxt_function_call", "modifier",
            "parenthesized_expression", "qualified_name", "return", "switch_block",
            "type_with_generics", "wildcard_import",
        ];
        validate_unused_kinds_audit(&Groovy, documented_unused)
            .expect("Groovy unused node kinds audit failed");
    }
}
