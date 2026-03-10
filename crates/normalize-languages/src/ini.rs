//! INI configuration file support.

use crate::{Language, LanguageSymbols};
use tree_sitter::Node;

/// INI language support.
pub struct Ini;

impl Language for Ini {
    fn name(&self) -> &'static str {
        "INI"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ini", "cfg", "conf", "properties"]
    }
    fn grammar_name(&self) -> &'static str {
        "ini"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
            .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
    }
}

impl LanguageSymbols for Ini {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Ini, documented_unused)
            .expect("INI unused node kinds audit failed");
    }
}
