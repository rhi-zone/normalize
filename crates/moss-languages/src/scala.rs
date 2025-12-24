//! Scala language support.

use crate::{Export, LanguageSupport, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use moss_core::tree_sitter::Node;

/// Scala language support.
pub struct Scala;

impl LanguageSupport for Scala {
    fn name(&self) -> &'static str { "Scala" }
    fn extensions(&self) -> &'static [&'static str] { &["scala", "sc"] }
    fn grammar_name(&self) -> &'static str { "scala" }

    fn container_kinds(&self) -> &'static [&'static str] { &["class_definition", "object_definition", "trait_definition"] }
    fn function_kinds(&self) -> &'static [&'static str] { &["function_definition"] }
    fn type_kinds(&self) -> &'static [&'static str] { &["class_definition", "trait_definition"] }
    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_declaration"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["class_definition", "object_definition", "trait_definition", "function_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AccessModifier
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        // Scala: public by default, check for private/protected modifiers
        // TODO: implement proper visibility checking for Scala
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "class_definition" => SymbolKind::Class,
            "object_definition" => SymbolKind::Module,
            "trait_definition" => SymbolKind::Trait,
            "function_definition" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_expression",
            "block",
            "lambda_expression",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "do_while_expression",
            "try_expression",
            "return_expression",
            "throw_expression",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "case_clause",
            "for_expression",
            "while_expression",
            "do_while_expression",
            "try_expression",
            "catch_clause",
            "infix_expression", // for && and ||
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "do_while_expression",
            "try_expression",
            "function_definition",
            "class_definition",
            "object_definition",
            "trait_definition",
            "block",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let params = node.child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());
        let ret = node.child_by_field_name("return_type")
            .map(|r| format!(": {}", &content[r.byte_range()]))
            .unwrap_or_default();

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container { SymbolKind::Method } else { SymbolKind::Function },
            signature: format!("def {}{}{}", name, params, ret),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "object_definition" => (SymbolKind::Module, "object"),
            "trait_definition" => (SymbolKind::Trait, "trait"),
            _ => (SymbolKind::Class, "class"),
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: None,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
        })
    }
}
