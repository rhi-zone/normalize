//! SPARQL query language support.

use crate::{Export, Import, Language, Symbol, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// SPARQL language support.
pub struct Sparql;

impl Language for Sparql {
    fn name(&self) -> &'static str {
        "SPARQL"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sparql", "rq"]
    }
    fn grammar_name(&self) -> &'static str {
        "sparql"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, _node: &Node, _content: &str) -> Vec<Export> {
        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[]
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[]
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
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // SPARQL has no imports
        String::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
            // Query parts
            "select_clause", "where_clause", "construct_query", "construct_template",
            "construct_triples", "triples_block",
            // Modifiers
            "solution_modifier", "order_clause", "limit_clause", "offset_clause",
            "limit_offset_clauses", "group_clause", "having_clause", "values_clause",
            // Dataset
            "dataset_clause", "default_graph_clause", "named_graph_clause",
            // Update
            "modify", "insert_clause", "delete_clause", "using_clause", "data_block",
            // Declarations
            "base_declaration", "prefix_declaration",
            // Expressions
            "expression_list", "binary_expression", "unary_expression",
            "bracketted_expression", "function_call", "build_in_function",
            "regex_expression", "substring_expression", "string_replace_expression",
        ];
        validate_unused_kinds_audit(&Sparql, documented_unused)
            .expect("SPARQL unused node kinds audit failed");
    }
}
