//! DOT/Graphviz language support.

use crate::{ContainerBody, Language};
use tree_sitter::Node;

/// DOT (Graphviz) language support.
pub struct Dot;

impl Language for Dot {
    fn name(&self) -> &'static str {
        "DOT"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dot", "gv"]
    }
    fn grammar_name(&self) -> &'static str {
        "dot"
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("id"))
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
            "block",      // Statement block (body)
            "identifier", // Node/edge identifiers
            "operator",   // Edge operators (-> or --)
        ];
        validate_unused_kinds_audit(&Dot, documented_unused)
            .expect("DOT unused node kinds audit failed");
    }
}
