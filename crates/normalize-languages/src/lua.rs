//! Lua language support.

use crate::{Import, Language, Visibility};
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

    fn signature_suffix(&self) -> &'static str {
        " end"
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                let text = &content[node.byte_range()];
                return text.lines().next().unwrap_or(text).trim().to_string();
            }
        };
        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());
        let text = &content[node.byte_range()];
        let is_local = text.trim_start().starts_with("local ");
        let keyword = if is_local {
            "local function"
        } else {
            "function"
        };
        format!("{} {}{}", keyword, name, params)
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
                let text = &content[sibling.byte_range()];
                // LuaDoc comments start with ---
                if let Some(line) = text.strip_prefix("---") {
                    let line = line.strip_prefix(' ').unwrap_or(line);
                    doc_lines.push(line.to_string());
                } else {
                    break;
                }
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        let joined = doc_lines.join("\n").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
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
