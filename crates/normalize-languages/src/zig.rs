//! Zig language support.

use crate::{Import, Language, Visibility};
use tree_sitter::Node;

/// Zig language support.
pub struct Zig;

impl Language for Zig {
    fn name(&self) -> &'static str {
        "Zig"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }
    fn grammar_name(&self) -> &'static str {
        "zig"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        // Look for @import("module")
        if node.kind() != "builtin_call_expression" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("@import") {
            return Vec::new();
        }

        // Extract the string argument
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string_literal" {
                let module = content[child.byte_range()].trim_matches('"').to_string();
                let is_relative = module.starts_with('.');
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative,
                    line: node.start_position().row + 1,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Zig: @import("module")
        format!("@import(\"{}\")", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        // Check for pub keyword before the declaration
        if let Some(prev) = node.prev_sibling() {
            let text = &content[prev.byte_range()];
            if text == "pub" {
                return Visibility::Public;
            }
        }
        // Also check if node starts with pub
        let text = &content[node.byte_range()];
        if text.starts_with("pub ") {
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // FnProto uses field "function" for the name identifier.
        // VarDecl uses field "variable_type_function" for the name identifier.
        let name_node = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("variable_type_function"))?;
        Some(&content[name_node.byte_range()])
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
        let documented_unused: &[&str] = &[
            // Zig grammar uses PascalCase node kinds
            "ArrayTypeStart", "BUILTINIDENTIFIER", "BitShiftOp", "BlockExpr",
            "BlockExprStatement", "BlockLabel", "BuildinTypeExpr", "ContainerDeclType",
            "ForArgumentsList", "ForExpr", "ForItem", "ForPrefix", "ForTypeExpr",
            "FormatSequence", "IDENTIFIER", "IfExpr", "IfPrefix", "IfTypeExpr",
            "LabeledStatement", "LabeledTypeExpr", "LoopExpr", "LoopStatement",
            "LoopTypeExpr", "ParamType", "PrefixTypeOp", "PtrTypeStart",
            "SliceTypeStart", "Statement", "SwitchCase", "WhileContinueExpr",
            "WhileExpr", "WhilePrefix", "WhileTypeExpr",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "ForStatement",
            "WhileStatement",
            "Block",
            "IfStatement",
        ];
        validate_unused_kinds_audit(&Zig, documented_unused)
            .expect("Zig unused node kinds audit failed");
    }
}
