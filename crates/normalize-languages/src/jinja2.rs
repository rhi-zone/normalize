//! Jinja2 template support.

use crate::{Language, LanguageSymbols};

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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }
}

impl LanguageSymbols for Jinja2 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Statements — macro_statement covered by tags.scm; for/if/call/elif covered by
            // complexity.scm (audit doesn't check complexity.scm so they stay here)
            "autoescape_statement", "block_statement", "call_statement", "debug_statement",
            "do_statement", "expression_statement", "extends_statement",
            "filter_block_statement", "for_statement", "from_statement", "if_statement",
            "import_statement", "include_statement", "raw_statement",
            "set_block_statement", "set_statement", "trans_statement", "with_statement",
            // Expressions — future: calls.scm will cover call_expression
            "add_expression", "and_expression", "attribute_expression", "call_expression",
            "comparison_expression", "concat_expression", "dict_expression",
            "filter_expression", "list_expression", "mul_expression", "not_expression",
            "or_expression", "parenthesized_expression", "power_expression",
            "subscript_expression", "ternary_expression", "test_expression",
            "unary_expression",
            // Clauses / auxiliary — elif_clause covered by complexity.scm
            "elif_clause", "else_clause", "for_else", "identifier", "identifier_tuple",
            "import_item", "import_list", "pluralize_clause", "with_assignment",
            "with_assignments",
        ];
        validate_unused_kinds_audit(&Jinja2, documented_unused)
            .expect("Jinja2 unused node kinds audit failed");
    }
}
