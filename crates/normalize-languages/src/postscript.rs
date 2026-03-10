//! PostScript support.

use crate::{ContainerBody, Language, LanguageSymbols};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
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

impl LanguageSymbols for PostScript {}

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
