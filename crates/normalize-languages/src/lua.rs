//! Lua language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn container_kinds(&self) -> &'static [&'static str] {
        &[] // Lua doesn't have traditional classes
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["function_call"] // require("module")
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "function_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention // local = private, global = public
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        // Non-local functions are public
        if node.kind() == "function_declaration"
            && let Some(name) = self.node_name(node, content)
        {
            return vec![Export {
                name: name.to_string(),
                kind: SymbolKind::Function,
                line: node.start_position().row + 1,
            }];
        }
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "do_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
            "return_statement",
            "break_statement",
            "goto_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elseif_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
            "and",
            "or",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
            "function_declaration",
            "function_definition",
        ]
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
            docstring: self.extract_docstring(node, content),
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Lua uses --- or --[[ ]] for documentation
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" {
                // LDoc style: ---
                if text.starts_with("---") {
                    let doc = text.strip_prefix("---").unwrap_or(text).trim();
                    if !doc.starts_with('@') {
                        return Some(doc.to_string());
                    }
                }
                // Block comment style: --[[ ]]
                if text.starts_with("--[[") {
                    let inner = text
                        .strip_prefix("--[[")
                        .unwrap_or(text)
                        .strip_suffix("]]")
                        .unwrap_or(text)
                        .trim();
                    if !inner.is_empty() {
                        return Some(inner.to_string());
                    }
                }
                break;
            }
            prev = sibling.prev_sibling();
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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

    fn is_public(&self, node: &Node, content: &str) -> bool {
        let text = &content[node.byte_range()];
        !text.trim_start().starts_with("local ")
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
            "assignment_statement", "binary_expression", "block",
            "bracket_index_expression", "dot_index_expression", "else_statement",
            "empty_statement", "expression_list", "for_generic_clause",
            "for_numeric_clause", "identifier", "label_statement",
            "method_index_expression", "parenthesized_expression", "table_constructor",
            "unary_expression", "vararg_expression", "variable_declaration",
        ];
        validate_unused_kinds_audit(&Lua, documented_unused)
            .expect("Lua unused node kinds audit failed");
    }
}
