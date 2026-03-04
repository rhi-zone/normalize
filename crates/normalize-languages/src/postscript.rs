//! PostScript support.

use crate::{ContainerBody, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// PostScript language support.
pub struct PostScript;

impl Language for PostScript {
    fn name(&self) -> &'static str {
        "PostScript"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ps", "eps"]
    }
    fn grammar_name(&self) -> &'static str {
        "postscript"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "procedure" {
            return None;
        }

        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_function(node, content, false)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // PostScript procedure is itself "{ ... }"; no named body field
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // procedure node spans "{ ... }" directly
        crate::body::analyze_brace_body(body_node, content, inner_indent)
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
            "operator", "document_structure_comment",
        ];
        validate_unused_kinds_audit(&PostScript, documented_unused)
            .expect("PostScript unused node kinds audit failed");
    }
}
