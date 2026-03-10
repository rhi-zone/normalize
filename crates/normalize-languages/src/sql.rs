//! SQL language support.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// SQL language support.
pub struct Sql;

impl Language for Sql {
    fn name(&self) -> &'static str {
        "SQL"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sql"]
    }
    fn grammar_name(&self) -> &'static str {
        "sql"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        // SQL: use first line as signature (CREATE TABLE foo...)
        let text = &content[node.byte_range()];
        text.lines().next().unwrap_or(text).trim().to_string()
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

impl LanguageSymbols for Sql {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "alter_type", "array_size_definition", "between_expression", "binary_expression",
            "block", "column_definition", "column_definitions", "comment_statement",
            "drop_function", "drop_type", "enum", "enum_elements", "filter_expression",
            "frame_definition", "function_argument", "function_arguments",
            "function_body", "function_cost", "function_declaration", "function_language",
            "function_leakproof", "function_rows", "function_safety", "function_security",
            "function_strictness", "function_support", "function_volatility",
            "keyword_before", "keyword_case", "keyword_else", "keyword_enum",
            "keyword_except", "keyword_for", "keyword_force", "keyword_force_not_null",
            "keyword_force_null", "keyword_force_quote", "keyword_foreign",
            "keyword_format", "keyword_function", "keyword_geometry", "keyword_if",
            "keyword_match", "keyword_matched", "keyword_modify", "keyword_regclass",
            "keyword_regtype", "keyword_return", "keyword_returning", "keyword_returns",
            "keyword_statement", "keyword_type", "keyword_while", "keyword_with",
            "keyword_without", "modify_column", "parenthesized_expression",
            "reset_statement", "returning", "row_format", "select_expression",
            "set_statement", "statement", "unary_expression", "var_declaration",
            "var_declarations", "when_clause", "while_statement", "window_clause",
            "window_function", "window_specification",
            // Control flow in SQL procedural code — not definition kinds
            "case",
        ];
        validate_unused_kinds_audit(&Sql, documented_unused)
            .expect("SQL unused node kinds audit failed");
    }
}
