//! Common Lisp language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// Common Lisp language support.
pub struct CommonLisp;

impl Language for CommonLisp {
    fn name(&self) -> &'static str {
        "Common Lisp"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["lisp", "lsp", "cl", "asd"]
    }
    fn grammar_name(&self) -> &'static str {
        "commonlisp"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // (defpackage ...), (defclass ...), etc.
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // (defun ...), (defmacro ...), etc.
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // (defstruct ...), (defclass ...)
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // (require ...), (use-package ...)
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["list_lit"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport // (export ...)
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "list_lit" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // (defun name ...), (defmacro name ...), etc.
        for prefix in &["(defun ", "(defmacro ", "(defgeneric ", "(defmethod "] {
            if let Some(rest) = text.strip_prefix(prefix)
                && let Some(name) = rest.split_whitespace().next()
            {
                return vec![Export {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line,
                }];
            }
        }

        for prefix in &["(defclass ", "(defstruct "] {
            if let Some(rest) = text.strip_prefix(prefix)
                && let Some(name) = rest.split_whitespace().next()
            {
                return vec![Export {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // let, flet, labels, lambda
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["list_lit"] // if, cond, case, when, unless
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["list_lit"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["list_lit"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        if node.kind() != "list_lit" {
            return None;
        }

        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        for prefix in &["(defun ", "(defmacro ", "(defgeneric ", "(defmethod "] {
            if let Some(rest) = text.strip_prefix(prefix)
                && let Some(name) = rest.split_whitespace().next()
            {
                return Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    signature: first_line.trim().to_string(),
                    docstring: self.extract_docstring(node, content),
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

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "list_lit" {
            return None;
        }

        let text = &content[node.byte_range()];

        if let Some(rest) = text.strip_prefix("(defpackage ") {
            let name = rest.split_whitespace().next()?;
            return Some(Symbol {
                name: name.to_string(),
                kind: SymbolKind::Module,
                signature: format!("(defpackage {})", name),
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

        for prefix in &["(defclass ", "(defstruct "] {
            if let Some(rest) = text.strip_prefix(prefix) {
                let name = rest.split_whitespace().next()?;
                return Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    signature: format!("{}{}", prefix.trim_start_matches('('), name),
                    docstring: self.extract_docstring(node, content),
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

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Common Lisp docstrings are strings after the argument list
        let text = &content[node.byte_range()];
        // Simple heuristic: find first quoted string
        if let Some(start) = text.find('"')
            && let Some(end) = text[start + 1..].find('"')
        {
            return Some(text[start + 1..start + 1 + end].to_string());
        }
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list_lit" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        for prefix in &["(require ", "(use-package ", "(ql:quickload "] {
            if let Some(rest) = text.strip_prefix(prefix) {
                let module = rest
                    .split(|c: char| c.is_whitespace() || c == ')')
                    .next()
                    .map(|s| s.trim_matches(|c| c == '\'' || c == ':' || c == '"'))
                    .unwrap_or("")
                    .to_string();

                if !module.is_empty() {
                    return vec![Import {
                        module,
                        names: Vec::new(),
                        alias: None,
                        is_wildcard: false,
                        is_relative: false,
                        line,
                    }];
                }
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Common Lisp: (use-package :package) or (use-package :package (:import-from #:a #:b))
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(use-package :{})", import.module)
        } else {
            let symbols: Vec<String> = names_to_use.iter().map(|n| format!("#:{}", n)).collect();
            format!(
                "(use-package :{} (:import-from {}))",
                import.module,
                symbols.join(" ")
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

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
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
            // Loop-related clauses
            "accumulation_clause", "condition_clause", "do_clause", "for_clause",
            "for_clause_word", "loop_clause", "loop_macro", "repeat_clause",
            "termination_clause", "while_clause", "with_clause",
            // Format string specifiers
            "format_directive_type", "format_modifiers", "format_prefix_parameters",
            "format_specifier",
            // Comments
            "block_comment",
        ];
        validate_unused_kinds_audit(&CommonLisp, documented_unused)
            .expect("Common Lisp unused node kinds audit failed");
    }
}
