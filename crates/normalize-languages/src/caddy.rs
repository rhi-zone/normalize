//! Caddyfile configuration support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Caddy language support.
pub struct Caddy;

impl Language for Caddy {
    fn name(&self) -> &'static str {
        "Caddy"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["caddyfile"]
    }
    fn grammar_name(&self) -> &'static str {
        "caddy"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Caddy: import path
        format!("import {}", import.module)
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
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        if let Some(first_child) = node.children(&mut cursor).next() {
            return Some(&content[first_child.byte_range()]);
        }
        None
    }
}

impl LanguageSymbols for Caddy {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Matcher-related (matcher_name, matcher_token, matcher_definition used in tags.scm)
            "matcher_path", "matcher_path_regexp",
            "standard_matcher", "uri_path_with_placeholders",
            // Directive-related
            "directive_import", "directive_request_body", "request_body_option_max_size",
            "fastcgi_option_try_files", "encode_format", "log_option_format",
            // Blocks
            "global_options_block",
            // structural node, not extracted as symbols
            "directive_block",
        ];
        validate_unused_kinds_audit(&Caddy, documented_unused)
            .expect("Caddy unused node kinds audit failed");
    }
}
