//! Assembly language support.

use crate::{Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Assembly language support.
pub struct Asm;

impl Language for Asm {
    fn name(&self) -> &'static str {
        "Assembly"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["asm", "s", "S"]
    }
    fn grammar_name(&self) -> &'static str {
        "asm"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new() // asm grammar doesn't have imports
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // Labels end with ':'
        let text = &content[node.byte_range()];
        let name = text.trim().trim_end_matches(':');
        if !name.is_empty() { Some(name) } else { None }
    }
}

impl LanguageSymbols for Asm {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Asm instructions are too granular for symbol extraction
            "instruction",
            // Comments
            "block_comment",
        ];
        validate_unused_kinds_audit(&Asm, documented_unused)
            .expect("Assembly unused node kinds audit failed");
    }
}
