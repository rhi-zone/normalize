//! Tree-sitter query language support.

use crate::Language;

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
