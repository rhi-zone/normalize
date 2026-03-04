//! Lua language support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// Lua language support.
pub struct Lua;

impl Language for Lua {
    fn name(&self) -> &'static str {
        "Lua"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }
    fn grammar_name(&self) -> &'static str {
        "lua"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn signature_suffix(&self) -> &'static str {
        " end"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        // Check if function text starts with "local"
        let text = &content[node.byte_range()];
        let is_local = text.trim_start().starts_with("local ");
        let keyword = if is_local {
            "local function"
        } else {
            "function"
        };
        let signature = format!("{} {}{}", keyword, name, params);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature,
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if is_local {
                Visibility::Private
            } else {
                Visibility::Public
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        // Look for require("module") calls
        if node.kind() != "function_call" {
            return Vec::new();
        }

        let func_name = node
            .child_by_field_name("name")
            .map(|n| &content[n.byte_range()]);

        if func_name != Some("require") {
            return Vec::new();
        }

        if let Some(args) = node.child_by_field_name("arguments") {
            let mut cursor = args.walk();
            for child in args.children(&mut cursor) {
                if child.kind() == "string" {
                    let module = content[child.byte_range()]
                        .trim_matches(|c| c == '"' || c == '\'' || c == '[' || c == ']')
                        .to_string();
                    return vec![Import {
                        module,
                        names: Vec::new(),
                        alias: None,
                        is_wildcard: false,
                        is_relative: false,
                        line: node.start_position().row + 1,
                    }];
                }
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Lua: require("module")
        format!("require(\"{}\")", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.trim_start().starts_with("local ") {
            Visibility::Private
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[ "binary_expression", "block",
            "bracket_index_expression", "else_statement",
            "empty_statement", "for_generic_clause",
            "for_numeric_clause", "identifier", "label_statement", "parenthesized_expression", "table_constructor",
            "unary_expression", "vararg_expression", "variable_declaration",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "return_statement",
            "while_statement",
            "elseif_statement",
            "for_statement",
            "goto_statement",
            "do_statement",
            "if_statement",
            "break_statement",
            "repeat_statement",
            "function_call",
        ];
        validate_unused_kinds_audit(&Lua, documented_unused)
            .expect("Lua unused node kinds audit failed");
    }
}
