//! Uiua array programming language support.

use crate::{Import, Language, Symbol, Visibility};
use tree_sitter::Node;

/// Uiua language support.
pub struct Uiua;

impl Language for Uiua {
    fn name(&self) -> &'static str {
        "Uiua"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ua"]
    }
    fn grammar_name(&self) -> &'static str {
        "uiua"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn signature_suffix(&self) -> &'static str {
        ""
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
        // Uiua has no import mechanism
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[]
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
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
            // Functions and modifiers
            "function", "inlineFunction", "switchFunctions",
            "modifier1", "modifier2",
            // Other
            "module", "identifier", "identifierDeprecated", "formatter",
        ];
        validate_unused_kinds_audit(&Uiua, documented_unused)
            .expect("Uiua unused node kinds audit failed");
    }
}
