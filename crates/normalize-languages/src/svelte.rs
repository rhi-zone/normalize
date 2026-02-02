//! Svelte language support.

use crate::component::extract_embedded_content;
use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Svelte language support.
pub struct Svelte;

impl Language for Svelte {
    fn name(&self) -> &'static str {
        "Svelte"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["svelte"]
    }
    fn grammar_name(&self) -> &'static str {
        "svelte"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["script_element", "style_element"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[] // JS functions are in embedded script, not Svelte grammar
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &[] // JS imports are in embedded script, not Svelte grammar
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[] // JS exports are in embedded script, not Svelte grammar
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        // Look for export let/const/function
        let text = &content[node.byte_range()];

        if node.kind() == "export_statement" || text.contains("export ") {
            if let Some(name) = self.node_name(node, content) {
                let kind = if text.contains("function") {
                    SymbolKind::Function
                } else {
                    SymbolKind::Variable
                };

                return vec![Export {
                    name: name.to_string(),
                    kind,
                    line: node.start_position().row + 1,
                }];
            }
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["if_statement", "each_statement", "await_statement"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["if_statement", "each_statement", "await_statement"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["if_statement", "each_statement", "else_if_block"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "each_statement",
            "await_statement",
            "script_element",
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

    fn extract_container(&self, node: &Node, _content: &str) -> Option<Symbol> {
        let kind = match node.kind() {
            "script_element" => SymbolKind::Module,
            "style_element" => SymbolKind::Class,
            _ => return None,
        };

        let name = if node.kind() == "script_element" {
            "<script>".to_string()
        } else {
            "<style>".to_string()
        };

        Some(Symbol {
            name: name.clone(),
            kind,
            signature: name,
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // JavaScript-style comments
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" {
                if text.starts_with("/**") {
                    let inner = text
                        .strip_prefix("/**")
                        .unwrap_or(text)
                        .strip_suffix("*/")
                        .unwrap_or(text);
                    let lines: Vec<&str> = inner
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty() && !l.starts_with('@'))
                        .collect();
                    if !lines.is_empty() {
                        return Some(lines.join(" "));
                    }
                } else if text.starts_with("//") {
                    doc_lines.push(text.strip_prefix("//").unwrap_or(text).trim().to_string());
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
        if node.kind() != "import_statement" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract from clause
        if let Some(from_idx) = text.find(" from ") {
            let rest = &text[from_idx + 6..];
            if let Some(start) = rest.find('"').or_else(|| rest.find('\'')) {
                let quote = rest.chars().nth(start).unwrap();
                let inner = &rest[start + 1..];
                if let Some(end) = inner.find(quote) {
                    let module = inner[..end].to_string();
                    return vec![Import {
                        module: module.clone(),
                        names: Vec::new(),
                        alias: None,
                        is_wildcard: text.contains(" * "),
                        is_relative: module.starts_with('.'),
                        line,
                    }];
                }
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Svelte uses JS import syntax
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import '{}';", import.module)
        } else {
            format!(
                "import {{ {} }} from '{}';",
                names_to_use.join(", "),
                import.module
            )
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        let text = &content[node.byte_range()];
        text.contains("export ")
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
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                name.starts_with("test_")
                    || name.starts_with("Test")
                    || name == "describe"
                    || name == "it"
                    || name == "test"
            }
            crate::SymbolKind::Module => name == "tests" || name == "test" || name == "__tests__",
            _ => false,
        }
    }

    fn embedded_content(&self, node: &Node, content: &str) -> Option<crate::EmbeddedBlock> {
        extract_embedded_content(node, content)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Find the content of script/style elements
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "raw_text" {
                return Some(child);
            }
        }
        None
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("function"))
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
            "await_end", "await_start", "block_end_tag", "block_start_tag",
            "block_tag", "catch_block", "catch_start", "doctype", "else_block",
            "else_if_start", "else_start", "expression", "expression_tag",
            "if_end", "if_start", "key_statement", "snippet_statement", "then_block",
        ];
        validate_unused_kinds_audit(&Svelte, documented_unused)
            .expect("Svelte unused node kinds audit failed");
    }
}
