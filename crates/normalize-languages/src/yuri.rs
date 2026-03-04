//! Yuri language support (tree-sitter-yuri).

use crate::{Import, Language, Symbol, Visibility};
use tree_sitter::Node;

/// Yuri language support.
pub struct Yuri;

impl Language for Yuri {
    fn name(&self) -> &'static str {
        "Yuri"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["yuri"]
    }
    fn grammar_name(&self) -> &'static str {
        "yuri"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }
    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Yuri has no import mechanism
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
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
            // Items
            "function_item", "function_parameters", "module_item", "import_item",
            // Types
            "type_alias_item", "compound_type_item", "compound_type_field",
            "array_type_item", "primitive_type",
            // Statements
            "break_statement", "continue_statement", "return_statement",
            "else_clause",
            // Expressions
            "if_expression", "binary_expression", "unary_expression",
            "call_expression", "paren_expression", "array_expression",
            "compound_value_expression",
            // Other
            "block", "identifier",
        ];
        validate_unused_kinds_audit(&Yuri, documented_unused)
            .expect("Yuri unused node kinds audit failed");
    }
}
