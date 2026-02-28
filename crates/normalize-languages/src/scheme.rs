//! Scheme language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Scheme language support.
pub struct Scheme;

impl Language for Scheme {
    fn name(&self) -> &'static str {
        "Scheme"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scm", "ss", "rkt"]
    }
    fn grammar_name(&self) -> &'static str {
        "scheme"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["list"] // (define-library ...), (module ...)
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["list"] // (define (name args) ...)
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["list"] // (define-record-type ...)
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["list"] // (import ...), (require ...)
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["list"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "list" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // (define name ...) or (define (name args) ...)
        if let Some(rest) = text.strip_prefix("(define ") {
            let name = if let Some(inner) = rest.strip_prefix('(') {
                // (define (name args) ...)
                inner.split_whitespace().next()
            } else {
                // (define name ...)
                rest.split_whitespace().next()
            };

            if let Some(name) = name {
                let kind = if rest.starts_with('(') || rest.contains("(lambda") {
                    SymbolKind::Function
                } else {
                    SymbolKind::Variable
                };

                return vec![Export {
                    name: name.to_string(),
                    kind,
                    line,
                }];
            }
        }

        if let Some(rest) = text.strip_prefix("(define-syntax ")
            && let Some(name) = rest.split_whitespace().next()
        {
            return vec![Export {
                name: name.to_string(),
                kind: SymbolKind::Function,
                line,
            }];
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["list"] // let, let*, letrec, lambda
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["list"] // if, cond, case, when
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["list"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["list"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "list" {
            return None;
        }

        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        if let Some(rest) = text.strip_prefix("(define ") {
            // Only extract function definitions
            if rest.starts_with('(') || rest.contains("(lambda") {
                let name = if let Some(inner) = rest.strip_prefix('(') {
                    inner.split_whitespace().next()
                } else {
                    rest.split_whitespace().next()
                }?;

                return Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    signature: first_line.trim().to_string(),
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

        if let Some(rest) = text.strip_prefix("(define-syntax ") {
            let name = rest.split_whitespace().next()?;
            return Some(Symbol {
                name: name.to_string(),
                kind: SymbolKind::Function,
                signature: first_line.trim().to_string(),
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

        None
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "list" {
            return None;
        }

        let text = &content[node.byte_range()];

        if text.starts_with("(define-library ")
            || text.starts_with("(library ")
            || text.starts_with("(module ")
        {
            let prefix_len = if text.starts_with("(define-library ") {
                16
            } else if text.starts_with("(library ") {
                9
            } else {
                8
            };

            let name = text[prefix_len..]
                .split(|c: char| c.is_whitespace() || c == ')')
                .next()?
                .to_string();

            return Some(Symbol {
                name: name.clone(),
                kind: SymbolKind::Module,
                signature: format!("(library {})", name),
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

        None
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

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        for prefix in &["(import ", "(require "] {
            if text.starts_with(prefix) {
                return vec![Import {
                    module: "import".to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Scheme: (import (library)) or (import (only (library) a b c))
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(import ({}))", import.module)
        } else {
            format!(
                "(import (only ({}) {}))",
                import.module,
                names_to_use.join(" ")
            )
        }
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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
        // list is itself "( ... )" — use node directly for paren analysis
        Some(*node)
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }
    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_paren_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
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
            "block_comment",
        ];
        validate_unused_kinds_audit(&Scheme, documented_unused)
            .expect("Scheme unused node kinds audit failed");
    }
}
