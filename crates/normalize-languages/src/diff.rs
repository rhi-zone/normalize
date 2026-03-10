//! Diff/patch file support.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// Diff language support.
pub struct Diff;

impl Language for Diff {
    fn name(&self) -> &'static str {
        "Diff"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["diff", "patch"]
    }
    fn grammar_name(&self) -> &'static str {
        "diff"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

impl LanguageSymbols for Diff {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block", // Hunk block
        ];
        validate_unused_kinds_audit(&Diff, documented_unused)
            .expect("Diff unused node kinds audit failed");
    }
}
