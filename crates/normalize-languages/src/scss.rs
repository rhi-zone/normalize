//! SCSS language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["rule_set", "mixin_statement", "function_statement"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["mixin_statement", "function_statement"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_statement", "use_statement", "forward_statement"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["mixin_statement", "function_statement"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention // _ prefix = private
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => return Vec::new(),
        };

        // _ prefix means private in SCSS
        if name.starts_with('_') {
            return Vec::new();
        }

        let kind = match node.kind() {
            "mixin_statement" | "function_statement" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        vec![Export {
            name: name.to_string(),
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["block", "rule_set"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "each_statement",
            "while_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "each_statement",
            "while_statement",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "rule_set",
            "mixin_statement",
            "function_statement",
            "if_statement",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "rule_set" {
            return self.extract_function(node, content, false);
        }

        // Extract selector
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "selectors" {
                let selector = content[child.byte_range()].to_string();
                return Some(Symbol {
                    name: selector.clone(),
                    kind: SymbolKind::Class,
                    signature: selector,
                    docstring: None,
                    attributes: Vec::new(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: Visibility::Public,
                    children: Vec::new(),
                    is_interface_impl: false,
                    implements: Vec::new(),
                });
            }
        }

        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // SCSS uses /// for SassDoc
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" && text.starts_with("///") {
                let line = text.strip_prefix("///").unwrap_or(text).trim();
                if !line.starts_with('@') {
                    doc_lines.push(line.to_string());
                }
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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

    fn is_public(&self, node: &Node, content: &str) -> bool {
        if let Some(name) = self.node_name(node, content) {
            !name.starts_with('_')
        } else {
            true
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self.is_public(node, content) {
            Visibility::Public
        } else {
            Visibility::Private
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
            .or_else(|| node.child_by_field_name("block"))
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
        ];
        validate_unused_kinds_audit(&Scss, documented_unused)
            .expect("SCSS unused node kinds audit failed");
    }
}
