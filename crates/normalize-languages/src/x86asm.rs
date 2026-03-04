//! x86 assembly support.

use crate::{Language, Symbol, Visibility};
use tree_sitter::Node;

/// x86 Assembly language support.
pub struct X86Asm;

impl Language for X86Asm {
    fn name(&self) -> &'static str {
        "x86 Assembly"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["asm", "s", "S"]
    }
    fn grammar_name(&self) -> &'static str {
        "x86asm"
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }
    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
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
            "label_definition", "instruction", "identifier",
            "memory_expression", "binary_expression",
        ];
        validate_unused_kinds_audit(&X86Asm, documented_unused)
            .expect("x86 Assembly unused node kinds audit failed");
    }
}
