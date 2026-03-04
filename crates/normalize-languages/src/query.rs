//! Tree-sitter query language support.

use crate::{Import, Language, Symbol, Visibility};
use tree_sitter::Node;

/// Tree-sitter query language support.
pub struct Query;

impl Language for Query {
    fn name(&self) -> &'static str {
        "Query"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scm"]
    }
    fn grammar_name(&self) -> &'static str {
        "query"
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
        // Tree-sitter query has no imports
        String::new()
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
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
            "identifier", "quantifier", "field_definition", "predicate_type",
        ];
        validate_unused_kinds_audit(&Query, documented_unused)
            .expect("Query unused node kinds audit failed");
    }
}
