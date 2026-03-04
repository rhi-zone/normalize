//! AWK language support.

use crate::{ContainerBody, Import, Language, Symbol, Visibility, simple_function_symbol};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn signature_suffix(&self) -> &'static str {
        ""
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

    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // AWK has no imports
        String::new()
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[]
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
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
