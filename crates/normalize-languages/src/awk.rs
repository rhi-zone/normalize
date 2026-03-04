//! AWK language support.

use crate::{Language, Symbol, Visibility, simple_function_symbol};
use tree_sitter::Node;

/// AWK language support.
pub struct Awk;

impl Language for Awk {
    fn name(&self) -> &'static str {
        "AWK"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["awk", "gawk"]
    }
    fn grammar_name(&self) -> &'static str {
        "awk"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(node, content, name, None))
    }

    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
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
            "break_statement", "continue_statement", "delete_statement", "do_while_statement",
            "else_clause", "exit_statement", "identifier", "next_statement", "nextfile_statement",
            "ns_qualified_name", "piped_io_statement", "print_statement", "printf_statement",
            "redirected_io_statement", "return_statement", "switch_body", "switch_case",
            "switch_statement",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "if_statement",
            "for_in_statement",
            "for_statement",
            "while_statement",
            "block",
        ];
        validate_unused_kinds_audit(&Awk, documented_unused)
            .expect("AWK unused node kinds audit failed");
    }
}
