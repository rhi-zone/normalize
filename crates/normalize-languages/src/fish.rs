//! Fish shell language support.

use crate::{Import, Language, Symbol, Visibility, simple_function_symbol};
use tree_sitter::Node;

/// Fish shell language support.
pub struct Fish;

impl Language for Fish {
    fn name(&self) -> &'static str {
        "Fish"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fish"]
    }
    fn grammar_name(&self) -> &'static str {
        "fish"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(node, content, name, None))
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("source ") {
            return Vec::new();
        }

        let module = text.strip_prefix("source ").map(|s| s.trim().to_string());

        if let Some(module) = module {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: true,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Fish: source file
        format!("source {}", import.module)
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
            "else_clause", "negated_statement", "redirect_statement", "return",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "begin_statement",
            "switch_statement",
            "for_statement",
            "case_clause",
            "if_statement",
            "else_if_clause",
            "while_statement",
        ];
        validate_unused_kinds_audit(&Fish, documented_unused)
            .expect("Fish unused node kinds audit failed");
    }
}
