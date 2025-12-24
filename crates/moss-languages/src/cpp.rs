//! C++ language support.

use crate::{LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use moss_core::tree_sitter::Node;

/// C++ language support.
pub struct Cpp;

impl LanguageSupport for Cpp {
    fn name(&self) -> &'static str { "C++" }
    fn extensions(&self) -> &'static [&'static str] { &["cpp", "cc", "cxx", "hpp", "hh", "hxx"] }
    fn grammar_name(&self) -> &'static str { "cpp" }

    fn container_kinds(&self) -> &'static [&'static str] { &["class_specifier", "struct_specifier"] }
    fn function_kinds(&self) -> &'static [&'static str] { &["function_definition"] }
    fn type_kinds(&self) -> &'static [&'static str] { &["class_specifier", "struct_specifier", "enum_specifier", "type_definition"] }
    fn import_kinds(&self) -> &'static [&'static str] { &["preproc_include"] }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "class_specifier", "struct_specifier"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::HeaderBased // Also has public/private in classes, but header-based is primary
    }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "for_range_loop",
            "while_statement",
            "compound_statement",
            "lambda_expression",
            "namespace_definition",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "switch_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "throw_statement",
            "goto_statement",
            "try_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "switch_statement",
            "case_statement",
            "try_statement",
            "catch_clause",
            "throw_statement",
            "&&",
            "||",
            "conditional_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "function_definition",
            "class_specifier",
            "struct_specifier",
            "namespace_definition",
            "lambda_expression",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let declarator = node.child_by_field_name("declarator")?;
        let name = find_identifier(&declarator, content)?;

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container { SymbolKind::Method } else { SymbolKind::Function },
            signature: name.to_string(),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let kind = if node.kind() == "class_specifier" { SymbolKind::Class } else { SymbolKind::Struct };

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

fn find_identifier<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    if node.kind() == "identifier" || node.kind() == "field_identifier" {
        return Some(&content[node.byte_range()]);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(id) = find_identifier(&child, content) {
            return Some(id);
        }
    }
    None
}
