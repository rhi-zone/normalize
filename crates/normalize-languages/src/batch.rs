//! Windows Batch file support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Batch language support.
pub struct Batch;

impl Language for Batch {
    fn name(&self) -> &'static str {
        "Batch"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["bat", "cmd"]
    }
    fn grammar_name(&self) -> &'static str {
        "batch"
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Batch: call script.bat
        format!("call {}", import.module)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
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
            "identifier",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "function_definition",
            "variable_declaration",
        ];
        validate_unused_kinds_audit(&Batch, documented_unused)
            .expect("Batch unused node kinds audit failed");
    }
}
