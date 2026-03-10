//! Markdown language support.

use crate::{ContainerBody, Language, LanguageSymbols};
use tree_sitter::Node;

/// Markdown language support.
pub struct Markdown;

impl Language for Markdown {
    fn name(&self) -> &'static str {
        "Markdown"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown"]
    }
    fn grammar_name(&self) -> &'static str {
        "markdown"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    // Markdown sections are modeled as `section` nodes in the grammar,
    // each containing an atx_heading as the first child followed by content blocks.

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // The section node itself contains the heading + content as children.
        // Returning it here lets collect_symbols recurse into child sections.
        Some(*node)
    }
    fn analyze_container_body(
        &self,
        section_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        // Skip the first child (atx_heading); body is everything after it.
        let mut cursor = section_node.walk();
        let mut children = section_node.children(&mut cursor);
        children.next(); // skip heading
        let content_start = children
            .next()
            .map(|n| n.start_byte())
            .unwrap_or(section_node.end_byte());
        let content_end = section_node.end_byte();
        Some(ContainerBody {
            content_start,
            content_end,
            inner_indent: String::new(),
            is_empty: content_start >= content_end,
        })
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // section → atx_heading → inline
        let heading = node.child(0).filter(|c| c.kind() == "atx_heading")?;
        let mut cursor = heading.walk();
        let inline = heading
            .children(&mut cursor)
            .find(|c| c.kind() == "inline")?;
        Some(content[inline.byte_range()].trim())
    }
}

impl LanguageSymbols for Markdown {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "block_continuation", "block_quote", "block_quote_marker",
            "fenced_code_block", "fenced_code_block_delimiter",
            "html_block", "indented_code_block", "link_reference_definition",
        ];
        // Note: section and atx_heading are captured via markdown.tags.scm (`@definition.heading`).

        validate_unused_kinds_audit(&Markdown, documented_unused)
            .expect("Markdown unused node kinds audit failed");
    }
}
