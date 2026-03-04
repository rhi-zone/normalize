//! Jinja2 template support.

use crate::{Import, Language, Symbol, Visibility};
use tree_sitter::Node;

/// Jinja2 language support.
pub struct Jinja2;

impl Language for Jinja2 {
    fn name(&self) -> &'static str {
        "Jinja2"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["j2", "jinja", "jinja2"]
    }
    fn grammar_name(&self) -> &'static str {
        "jinja2"
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
        // Jinja2 grammar is minimal - only basic tokens, no structured nodes
        None
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        // Jinja2 grammar is minimal - only basic tokens, no structured nodes
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        // Jinja2 grammar is minimal - only basic tokens, no structured nodes
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Jinja2 has no imports
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
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
            // This grammar is minimal - only basic tokens, no structured blocks/macros
            "identifier", "expression", "statement", "operator",
            "expression_begin", "expression_end", "statement_begin", "statement_end",
        ];
        validate_unused_kinds_audit(&Jinja2, documented_unused)
            .expect("Jinja2 unused node kinds audit failed");
    }
}
