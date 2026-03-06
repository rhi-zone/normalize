//! Jinja2 template support.

use crate::Language;

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
