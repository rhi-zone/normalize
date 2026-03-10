//! SPARQL query language support.

use crate::{Language, LanguageSymbols};

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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }
}

impl LanguageSymbols for Sparql {}

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
