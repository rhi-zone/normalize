//! Elixir language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["call"] // defmodule, defprotocol, defimpl
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["call"] // def, defp, defmacro, defmacrop
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["call"] // defstruct, @type
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["call"] // import, alias, require, use
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["call"] // def, defmacro (not defp, defmacrop)
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention // def = public, defp = private
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "call" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];

        // Check for def (not defp)
        if text.starts_with("def ")
            && !text.starts_with("defp")
            && let Some(name) = self.extract_def_name(node, content)
        {
            return vec![Export {
                name,
                kind: SymbolKind::Function,
                line: node.start_position().row + 1,
            }];
        }

        // Check for defmacro (not defmacrop)
        if text.starts_with("defmacro ")
            && !text.starts_with("defmacrop")
            && let Some(name) = self.extract_def_name(node, content)
        {
            return vec![Export {
                name,
                kind: SymbolKind::Function,
                line: node.start_position().row + 1,
            }];
        }

        // Check for defmodule
        if text.starts_with("defmodule ")
            && let Some(name) = self.extract_module_name(node, content)
        {
            return vec![Export {
                name,
                kind: SymbolKind::Module,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["do_block", "anonymous_function"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["call"] // if, case, cond, with, for, try
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["call", "binary_operator"] // if, case, cond, and/or
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["call", "do_block", "anonymous_function"]
    }

    fn signature_suffix(&self) -> &'static str {
        " end"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "call" {
            return None;
        }

        let text = &content[node.byte_range()];
        let is_private = if text.starts_with("defp ") || text.starts_with("defmacrop ") {
            true
        } else if text.starts_with("def ") || text.starts_with("defmacro ") {
            false
        } else {
            return None;
        };

        let name = self.extract_def_name(node, content)?;

        // Extract first line as signature
        let first_line = text.lines().next().unwrap_or(text);
        let signature = first_line.trim_end_matches(" do").to_string();

        Some(Symbol {
            name,
            kind: SymbolKind::Function,
            signature,
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: if is_private {
                Visibility::Private
            } else {
                Visibility::Public
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "call" {
            return None;
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("defmodule ") {
            return None;
        }

        let name = self.extract_module_name(node, content)?;

        Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Module,
            signature: format!("defmodule {}", name),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Look for @doc or @moduledoc before the node
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if text.contains("@doc") || text.contains("@moduledoc") {
                // Extract the string content
                if let Some(start) = text.find("\"\"\"") {
                    let rest = &text[start + 3..];
                    if let Some(end) = rest.find("\"\"\"") {
                        return Some(rest[..end].trim().to_string());
                    }
                }
                if let Some(start) = text.find('"') {
                    let rest = &text[start + 1..];
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].to_string());
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

    fn is_public(&self, node: &Node, content: &str) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let text = &content[node.byte_range()];
        (text.starts_with("def ") && !text.starts_with("defp"))
            || (text.starts_with("defmacro ") && !text.starts_with("defmacrop"))
            || text.starts_with("defmodule ")
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self.is_public(node, content) {
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Look for do_block child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "do_block")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_do_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

impl Elixir {
    fn extract_def_name(&self, node: &Node, content: &str) -> Option<String> {
        // Look for the function name after def/defp
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call" || child.kind() == "identifier" {
                let text = &content[child.byte_range()];
                // Extract just the name (before parentheses)
                let name = text.split('(').next().unwrap_or(text).trim();
                if !name.is_empty()
                    && name != "def"
                    && name != "defp"
                    && name != "defmacro"
                    && name != "defmacrop"
                {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

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
        ];
        validate_unused_kinds_audit(&Elixir, documented_unused)
            .expect("Elixir unused node kinds audit failed");
    }
}
