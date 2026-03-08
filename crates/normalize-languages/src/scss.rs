//! SCSS language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// SCSS language support.
pub struct Scss;

impl Language for Scss {
    fn name(&self) -> &'static str {
        "SCSS"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scss", "sass"]
    }
    fn grammar_name(&self) -> &'static str {
        "scss"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Handle @import, @use, @forward
        for keyword in &["@import ", "@use ", "@forward "] {
            if let Some(stripped) = text.strip_prefix(keyword) {
                let rest = stripped.trim();
                // Extract quoted path
                if let Some(start) = rest.find('"').or_else(|| rest.find('\'')) {
                    // normalize-syntax-allow: rust/unwrap-in-impl - start is the byte offset of an ASCII quote char; byte == char index for ASCII
                    let quote = rest.chars().nth(start).unwrap();
                    let inner = &rest[start + 1..];
                    if let Some(end) = inner.find(quote) {
                        let module = inner[..end].to_string();
                        return vec![Import {
                            module,
                            names: Vec::new(),
                            alias: None,
                            is_wildcard: false,
                            is_relative: true,
                            line,
                        }];
                    }
                }
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // SCSS: @import "path" or @use "path"
        format!("@import \"{}\"", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if let Some(name) = self.node_name(node, content) {
            if name.starts_with('_') {
                Visibility::Private
            } else {
                Visibility::Public
            }
        } else {
            Visibility::Public
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
            .or_else(|| node.child_by_field_name("block"))
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        // Run cross_check_node_kinds to populate
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "at_root_statement", "binary_expression", "call_expression",
            "charset_statement", "class_name", "class_selector", "debug_statement",
            "declaration", "else_clause", "else_if_clause", "error_statement",
            "extend_statement", "function_name", "identifier", "important",
            "important_value", "include_statement", "keyframe_block",
            "keyframe_block_list", "keyframes_statement", "media_statement",
            "namespace_statement", "postcss_statement", "pseudo_class_selector",
            "return_statement", "scope_statement", "supports_statement", "warn_statement",
            // Control flow — not definition kinds
            "block", "each_statement", "for_statement", "if_statement", "while_statement",
            // Module system — handled in extract_imports, not as symbols
            "forward_statement", "import_statement", "use_statement",
            // control flow — not extracted as symbols

        ];
        validate_unused_kinds_audit(&Scss, documented_unused)
            .expect("SCSS unused node kinds audit failed");
    }
}
