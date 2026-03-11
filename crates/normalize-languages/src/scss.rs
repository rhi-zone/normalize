//! SCSS language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, SymbolKind, Visibility};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
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

    fn refine_kind(&self, node: &Node, _content: &str, tag_kind: SymbolKind) -> SymbolKind {
        match node.kind() {
            "media_statement" | "supports_statement" | "keyframes_statement" => SymbolKind::Module,
            _ => tag_kind,
        }
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        match node.kind() {
            "mixin_statement" | "function_statement" => {
                let name_node = node.child_by_field_name("name")?;
                Some(content[name_node.byte_range()].trim())
            }
            "rule_set" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "selectors" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            "media_statement" => extract_at_rule_name(node, content, "@media"),
            "supports_statement" => extract_at_rule_name(node, content, "@supports"),
            "keyframes_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "keyframes_name" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            "declaration" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "property_name" {
                        return Some(content[child.byte_range()].trim());
                    }
                }
                None
            }
            _ => node
                .child_by_field_name("name")
                .map(|n| content[n.byte_range()].trim()),
        }
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        match node.kind() {
            "rule_set" | "media_statement" | "supports_statement" | "keyframes_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" || child.kind() == "keyframe_block_list" {
                        return Some(child);
                    }
                }
                None
            }
            _ => node
                .child_by_field_name("body")
                .or_else(|| node.child_by_field_name("block")),
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if let Some(name) = self.node_name(node, content) {
            match node.kind() {
                "mixin_statement" => {
                    // Include parameters if present
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "parameters" {
                            let params = content[child.byte_range()].trim();
                            return format!(
                                "@mixin {}({}) {{ … }}",
                                name,
                                params.trim_matches(|c| c == '(' || c == ')')
                            );
                        }
                    }
                    format!("@mixin {} {{ … }}", name)
                }
                "function_statement" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "parameters" {
                            let params = content[child.byte_range()].trim();
                            return format!(
                                "@function {}({}) {{ … }}",
                                name,
                                params.trim_matches(|c| c == '(' || c == ')')
                            );
                        }
                    }
                    format!("@function {} {{ … }}", name)
                }
                "rule_set" => format!("{} {{ … }}", name),
                "media_statement" => format!("@media {} {{ … }}", name),
                "supports_statement" => format!("@supports {} {{ … }}", name),
                "keyframes_statement" => format!("@keyframes {} {{ … }}", name),
                "declaration" => {
                    let mut cursor = node.walk();
                    let mut found_name = false;
                    for child in node.children(&mut cursor) {
                        if child.kind() == "property_name" {
                            found_name = true;
                        } else if found_name && child.kind() != ":" && child.kind() != ";" {
                            let val = content[child.byte_range()].trim();
                            if val.len() > 40 {
                                return format!("{}: {}…", name, &val[..37]);
                            }
                            return format!("{}: {}", name, val);
                        }
                    }
                    name.to_string()
                }
                _ => name.to_string(),
            }
        } else {
            content[node.byte_range()]
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        }
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

impl LanguageSymbols for Scss {}

/// Extract the text between an at-rule keyword and its block.
fn extract_at_rule_name<'a>(node: &Node, content: &'a str, keyword: &str) -> Option<&'a str> {
    let full = &content[node.byte_range()];
    let after_keyword = full.strip_prefix(keyword)?.trim_start();
    let name = after_keyword.split('{').next()?.trim();
    if name.is_empty() {
        return None;
    }
    let start = node.start_byte() + full.find(name)?;
    let end = start + name.len();
    Some(&content[start..end])
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
            "else_clause", "else_if_clause", "error_statement",
            "extend_statement", "function_name", "identifier", "important",
            "important_value", "include_statement", "keyframe_block",
            "keyframe_block_list",
            "namespace_statement", "postcss_statement", "pseudo_class_selector",
            "return_statement", "scope_statement", "warn_statement",
            // Structural — used in container_body but not definition kinds
            "block",
            // Control flow — not definition kinds
            "each_statement", "for_statement", "if_statement", "while_statement",
            // Module system — handled in extract_imports, not as symbols
            "forward_statement", "import_statement", "use_statement",

        ];
        validate_unused_kinds_audit(&Scss, documented_unused)
            .expect("SCSS unused node kinds audit failed");
    }
}
