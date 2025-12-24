//! Ruby language support.

use crate::{LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use moss_core::tree_sitter::Node;

/// Ruby language support.
pub struct Ruby;

impl LanguageSupport for Ruby {
    fn name(&self) -> &'static str { "Ruby" }
    fn extensions(&self) -> &'static [&'static str] { &["rb"] }
    fn grammar_name(&self) -> &'static str { "ruby" }

    fn container_kinds(&self) -> &'static [&'static str] { &["class", "module"] }
    fn function_kinds(&self) -> &'static [&'static str] { &["method", "singleton_method"] }
    fn type_kinds(&self) -> &'static [&'static str] { &["class", "module"] }
    fn import_kinds(&self) -> &'static [&'static str] {
        &["call"] // require, require_relative, load are method calls
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["class", "module", "method", "singleton_method"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic // Ruby methods are public by default
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "do_block",
            "block",
            "lambda",
            "for",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if",
            "unless",
            "case",
            "while",
            "until",
            "for",
            "return",
            "break",
            "next",
            "redo",
            "retry",
            "raise",
            "begin",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if",
            "unless",
            "case",
            "when",
            "while",
            "until",
            "for",
            "begin", // rescue clauses
            "rescue",
            "and",
            "or",
            "conditional",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if",
            "unless",
            "case",
            "while",
            "until",
            "for",
            "begin",
            "method",
            "singleton_method",
            "class",
            "module",
            "do_block",
            "block",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Method,
            signature: format!("def {}", name),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let kind = if node.kind() == "module" { SymbolKind::Module } else { SymbolKind::Class };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", kind.as_str(), name),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }
}
