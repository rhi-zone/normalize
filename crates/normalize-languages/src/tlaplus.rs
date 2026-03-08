//! TLA+ specification language support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// TLA+ language support.
pub struct TlaPlus;

impl Language for TlaPlus {
    fn name(&self) -> &'static str {
        "TLA+"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tla"]
    }
    fn grammar_name(&self) -> &'static str {
        "tlaplus"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "extends" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // TLA+: EXTENDS ModuleName
        format!("EXTENDS {}", import.module)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // TLA+ module has no dedicated body field; use the module node itself
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // TLA+ module: ---- MODULE Foo ----\n...body...\n====
        // Skip the first line (---- MODULE Foo ----), strip ==== from the tail.
        let start = body_node.start_byte();
        let end = body_node.end_byte();
        let bytes = content.as_bytes();

        let mut content_start = start;
        while content_start < end && bytes[content_start] != b'\n' {
            content_start += 1;
        }
        if content_start < end {
            content_start += 1; // skip \n
        }

        let mut content_end = end;
        if end >= 4 && bytes.get(end - 4..end) == Some(b"====") {
            content_end = end - 4;
            while content_end > content_start
                && matches!(bytes[content_end - 1], b' ' | b'\t' | b'\n')
            {
                content_end -= 1;
            }
        }

        let is_empty = content[content_start..content_end].trim().is_empty();
        Some(ContainerBody {
            content_start,
            content_end,
            inner_indent: inner_indent.to_string(),
            is_empty,
        })
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
            // Declarations and definitions
            "constant_declaration", "variable_declaration", "operator_declaration",
            "local_definition", "function_definition", "module_definition",
            "recursive_declaration", "operator_args",
            // Proof-related
            "definition_proof_step", "case_proof_step", "use_body", "use_body_def",
            "use_body_expr", "statement_level",
            // Quantifiers
            "forall", "quantifier_bound", "tuple_of_identifiers",
            "unbounded_quantification", "bounded_quantification", "temporal_forall",
            // Case expressions
            "case_arm", "case_arrow", "case_box",
            // Function-related
            "function_literal", "function_evaluation", "set_of_functions",
            // Except
            "except", "except_update", "except_update_specifier",
            "except_update_fn_appl", "except_update_record_field",
            // PlusCal
            "pcal_algorithm_body", "pcal_definitions", "pcal_if", "pcal_end_if",
            "pcal_while", "pcal_end_while", "pcal_with", "pcal_end_with",
            "pcal_await", "pcal_return",
            // Identifiers and references
            "identifier", "identifier_ref", "module_ref",
            // Comments
            "block_comment", "block_comment_text",
            // Other
            "lambda", "iff", "format", "subexpression",
            // Control flow — not definition constructs
            "case", "if_then_else",
            // control flow — not extracted as symbols

        ];
        validate_unused_kinds_audit(&TlaPlus, documented_unused)
            .expect("TLA+ unused node kinds audit failed");
    }
}
