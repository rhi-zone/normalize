//! Shared utilities for HTML-based component languages (Vue, Svelte).
//!
//! These languages share similar structure: `<script>` and `<style>` blocks with optional
//! `lang` attributes to specify the embedded language (TypeScript, SCSS, etc.).

use tree_sitter::Node;

use crate::EmbeddedBlock;

/// Find the raw_text child of a script/style element.
pub fn find_raw_text_child<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|&child| child.kind() == "raw_text")
}

/// Detect script language from the lang attribute (e.g., `<script lang="ts">`).
pub fn detect_script_lang(node: &Node, content: &str) -> &'static str {
    if let Some(lang) = get_lang_attribute(node, content) {
        match lang {
            "ts" | "typescript" => return "typescript",
            "tsx" => return "tsx",
            _ => {}
        }
    }
    "javascript"
}

/// Detect style language from the lang attribute (e.g., `<style lang="scss">`).
pub fn detect_style_lang(node: &Node, content: &str) -> &'static str {
    if let Some(lang) = get_lang_attribute(node, content) {
        match lang {
            "scss" | "sass" => return "scss",
            "less" => return "css", // No less grammar, fall back to CSS
            _ => {}
        }
    }
    "css"
}

/// Get the lang attribute value from a script/style element.
pub fn get_lang_attribute<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Look for start_tag which contains the attributes
        if child.kind() == "start_tag" {
            let mut inner_cursor = child.walk();
            for attr in child.children(&mut inner_cursor) {
                if attr.kind() == "attribute" {
                    // Check if this is a lang attribute
                    let mut attr_cursor = attr.walk();
                    let mut is_lang = false;
                    for part in attr.children(&mut attr_cursor) {
                        if part.kind() == "attribute_name" {
                            let name = &content[part.byte_range()];
                            is_lang = name == "lang";
                        } else if is_lang && part.kind() == "quoted_attribute_value" {
                            // Get the value inside quotes
                            let value = &content[part.byte_range()];
                            return Some(value.trim_matches('"').trim_matches('\''));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract embedded content from a script or style element.
pub fn extract_embedded_content(node: &Node, content: &str) -> Option<EmbeddedBlock> {
    match node.kind() {
        "script_element" => {
            let raw = find_raw_text_child(node)?;
            let grammar = detect_script_lang(node, content);
            Some(EmbeddedBlock {
                grammar,
                content: content[raw.byte_range()].to_string(),
                start_line: raw.start_position().row + 1,
            })
        }
        "style_element" => {
            let raw = find_raw_text_child(node)?;
            let grammar = detect_style_lang(node, content);
            Some(EmbeddedBlock {
                grammar,
                content: content[raw.byte_range()].to_string(),
                start_line: raw.start_position().row + 1,
            })
        }
        _ => None,
    }
}
