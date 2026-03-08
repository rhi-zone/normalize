//! YAML language support.

use crate::Language;
use tree_sitter::Node;

/// YAML language support.
pub struct Yaml;

impl Language for Yaml {
    fn name(&self) -> &'static str {
        "YAML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }
    fn grammar_name(&self) -> &'static str {
        "yaml"
    }

    fn has_symbols(&self) -> bool {
        false
    }

    // YAML is data, not code - no functions/types/control flow

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
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block_mapping_pair", "block_node", "block_scalar",
            "block_sequence", "block_sequence_item",
            // structural node, not extracted as symbols
            "block_mapping",
        ];
        validate_unused_kinds_audit(&Yaml, documented_unused)
            .expect("YAML unused node kinds audit failed");
    }
}
