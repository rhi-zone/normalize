//! Markdown language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    // Markdown sections are modeled as `section` nodes in the grammar,
    // each containing an atx_heading as the first child followed by content blocks.
    fn container_kinds(&self) -> &'static [&'static str] {
        &["section"]
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NotApplicable
    }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[]
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        // `node` is a `section` node; its first child is `atx_heading`.
        let heading = node.child(0).filter(|c| c.kind() == "atx_heading")?;

        let mut cursor = heading.walk();
        let text = heading
            .children(&mut cursor)
            .find(|c| c.kind() == "inline")
            .map(|c| content[c.byte_range()].trim().to_string())
            .unwrap_or_default();

        if text.is_empty() {
            return None;
        }

        let mut cursor2 = heading.walk();
        let level = heading
            .children(&mut cursor2)
            .find_map(|c| match c.kind() {
                "atx_h1_marker" => Some(1),
                "atx_h2_marker" => Some(2),
                "atx_h3_marker" => Some(3),
                "atx_h4_marker" => Some(4),
                "atx_h5_marker" => Some(5),
                "atx_h6_marker" => Some(6),
                _ => None,
            })
            .unwrap_or(1);

        Some(Symbol {
            name: text.clone(),
            kind: SymbolKind::Heading,
            signature: format!("{} {}", "#".repeat(level), text),
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

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Markdown has no imports
        String::new()
    }
    fn extract_public_symbols(&self, _node: &Node, _content: &str) -> Vec<Export> {
        Vec::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // The section node itself contains the heading + content as children.
        // Returning it here lets collect_symbols recurse into child sections.
        Some(*node)
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
        // Note: section and atx_heading are now used via container_kinds/extract_container.

        validate_unused_kinds_audit(&Markdown, documented_unused)
            .expect("Markdown unused node kinds audit failed");
    }
}
