//! TOML language support.

use crate::Language;
use tree_sitter::Node;

/// TOML language support.
pub struct Toml;

impl Language for Toml {
    fn name(&self) -> &'static str {
        "TOML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }
    fn grammar_name(&self) -> &'static str {
        "toml"
    }

    // TOML is config, not code - no functions/types/control flow

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // TOML has no "interesting" unused kinds matching our patterns
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Toml, documented_unused)
            .expect("TOML unused node kinds audit failed");
    }
}
