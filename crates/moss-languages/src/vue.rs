//! Vue language support.

use crate::{LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use moss_core::tree_sitter::Node;

/// Vue language support.
pub struct Vue;

impl LanguageSupport for Vue {
    fn name(&self) -> &'static str { "Vue" }
    fn extensions(&self) -> &'static [&'static str] { &["vue"] }
    fn grammar_name(&self) -> &'static str { "vue" }

    fn container_kinds(&self) -> &'static [&'static str] { &["script_element"] }
    fn function_kinds(&self) -> &'static [&'static str] { &["function_declaration", "method_definition"] }
    fn type_kinds(&self) -> &'static [&'static str] { &[] }
    // Vue uses JS/TS inside <script> blocks - these are JS node kinds
    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_statement"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["export_statement"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "for_in_statement",
            "arrow_function",
            "function_expression",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "throw_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "case",
            "try_statement",
            "catch_clause",
            "&&",
            "||",
            "??",
            "ternary_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "function_declaration",
            "arrow_function",
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
