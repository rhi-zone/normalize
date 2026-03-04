//! SQL language support.

use crate::{Language, Symbol, SymbolKind, Visibility};
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

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.extract_sql_name(node, content)?;

        // Extract first line as signature
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name,
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.extract_sql_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "create_view" | "create_materialized_view" => (SymbolKind::Struct, "VIEW"),
            "create_schema" => (SymbolKind::Module, "SCHEMA"),
            _ => (SymbolKind::Struct, "TABLE"),
        };

        Some(Symbol {
            name: name.clone(),
            kind,
            signature: format!("CREATE {} {}", keyword, name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.extract_sql_name(node, content)?;

        Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Type,
            signature: format!("CREATE TYPE {}", name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

impl Sql {
    fn extract_sql_name(&self, node: &Node, content: &str) -> Option<String> {
        // Look for identifier after CREATE TABLE/VIEW/FUNCTION etc.
        let mut cursor = node.walk();
        let mut found_create = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "keyword" {
                let text = &content[child.byte_range()].to_uppercase();
                if text == "CREATE" {
                    found_create = true;
                }
            }
            if found_create && (child.kind() == "identifier" || child.kind() == "object_reference")
            {
                return Some(content[child.byte_range()].to_string());
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
            "alter_type", "array_size_definition", "between_expression", "binary_expression",
            "block", "column_definition", "column_definitions", "comment_statement",
            "drop_function", "drop_type", "enum", "enum_elements", "filter_expression",
            "frame_definition", "function_argument", "function_arguments",
            "function_body", "function_cost", "function_declaration", "function_language",
            "function_leakproof", "function_rows", "function_safety", "function_security",
            "function_strictness", "function_support", "function_volatility", "identifier",
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
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "create_type",
        ];
        validate_unused_kinds_audit(&Sql, documented_unused)
            .expect("SQL unused node kinds audit failed");
    }
}
