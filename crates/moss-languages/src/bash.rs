//! Bash language support.

use crate::{LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use moss_core::tree_sitter::Node;

/// Bash language support.
pub struct Bash;

impl LanguageSupport for Bash {
    fn name(&self) -> &'static str { "Bash" }
    fn extensions(&self) -> &'static [&'static str] { &["sh", "bash", "zsh"] }
    fn grammar_name(&self) -> &'static str { "bash" }

    fn container_kinds(&self) -> &'static [&'static str] { &[] }
    fn function_kinds(&self) -> &'static [&'static str] { &["function_definition"] }
    fn type_kinds(&self) -> &'static [&'static str] { &[] }
    fn import_kinds(&self) -> &'static [&'static str] { &[] }
    fn public_symbol_kinds(&self) -> &'static [&'static str] { &["function_definition"] }
    fn visibility_mechanism(&self) -> VisibilityMechanism { VisibilityMechanism::AllPublic }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "subshell",
            "command_substitution",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "until_statement",
            "case_statement",
            "return_statement",
            "exit_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "until_statement",
            "case_statement",
            "case_item",
            "pipeline", // | chains
            "list",     // && and || chains
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "until_statement",
            "case_statement",
            "function_definition",
            "subshell",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: format!("function {}", name),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
}
