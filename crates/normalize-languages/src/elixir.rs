//! Elixir language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Elixir language support.
pub struct Elixir;

impl Language for Elixir {
    fn name(&self) -> &'static str {
        "Elixir"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }
    fn grammar_name(&self) -> &'static str {
        "elixir"
    }

    fn signature_suffix(&self) -> &'static str {
        " end"
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "unary_operator" {
                let text = content[sibling.byte_range()].trim();
                if text.starts_with('@')
                    && !text.starts_with("@doc")
                    && !text.starts_with("@moduledoc")
                {
                    attrs.insert(0, text.to_string());
                }
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }
        attrs
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if node.kind() != "call" {
            let text = &content[node.byte_range()];
            return text.lines().next().unwrap_or(text).trim().to_string();
        }
        let text = &content[node.byte_range()];
        if text.starts_with("defmodule ")
            && let Some(name) = self.extract_module_name(node, content)
        {
            return format!("defmodule {}", name);
        }
        // For def/defp/defmacro: take first line, trim trailing " do"
        let first_line = text.lines().next().unwrap_or(text).trim();
        first_line.trim_end_matches(" do").to_string()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "call" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Handle import, alias, require, use
        for keyword in &["import ", "alias ", "require ", "use "] {
            if let Some(stripped) = text.strip_prefix(keyword) {
                let rest = stripped.trim();
                let module = rest
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .next()
                    .unwrap_or(rest)
                    .to_string();

                if !module.is_empty() {
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
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Elixir: import Module or import Module, only: [a: 1, b: 2]
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!(
                "import {}, only: [{}]",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if node.kind() != "call" {
            return Visibility::Private;
        }
        let text = &content[node.byte_range()];
        let is_public = (text.starts_with("def ") && !text.starts_with("defp"))
            || (text.starts_with("defmacro ") && !text.starts_with("defmacrop"))
            || text.starts_with("defmodule ");
        if is_public {
            Visibility::Public
        } else {
            Visibility::Private
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/test/**/*.exs", "**/*_test.exs"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Look for do_block child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "do_block")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_do_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if node.kind() != "call" {
            // Fall back to default (child_by_field_name("name"))
            return node
                .child_by_field_name("name")
                .map(|n| &content[n.byte_range()]);
        }
        // For Elixir call nodes (def/defp/defmodule/defmacro/defprotocol/defimpl):
        // - defmodule MathUtils → arguments > alias
        // - def add(a, b) → arguments > call > target > identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                let mut arg_cursor = child.walk();
                for arg in child.children(&mut arg_cursor) {
                    match arg.kind() {
                        // defmodule/defprotocol/defimpl: (arguments (alias) ...)
                        "alias" => return Some(&content[arg.byte_range()]),
                        // def/defp/defmacro: (arguments (call target: (identifier) ...) ...)
                        "call" => {
                            if let Some(target) = arg.child_by_field_name("target") {
                                return Some(&content[target.byte_range()]);
                            }
                        }
                        // def with no args: (arguments (identifier) ...)
                        "identifier" => return Some(&content[arg.byte_range()]),
                        _ => {}
                    }
                }
            }
        }
        None
    }
}

impl Elixir {
    fn extract_module_name(&self, node: &Node, content: &str) -> Option<String> {
        // Look for the module name after defmodule
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "alias" || child.kind() == "atom" {
                let text = &content[child.byte_range()];
                if !text.is_empty() && text != "defmodule" {
                    return Some(text.to_string());
                }
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
            "after_block", "block", "body", "catch_block", "charlist",
            "else_block", "identifier", "interpolation", "operator_identifier",
            "rescue_block", "sigil_modifiers", "stab_clause", "struct",
            "unary_operator",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "binary_operator",
            "do_block",
            "anonymous_function",
        ];
        validate_unused_kinds_audit(&Elixir, documented_unused)
            .expect("Elixir unused node kinds audit failed");
    }
}
