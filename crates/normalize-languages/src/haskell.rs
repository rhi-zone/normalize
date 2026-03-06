//! Haskell language support.

use crate::{ContainerBody, Import, Language};
use tree_sitter::Node;

/// Haskell language support.
pub struct Haskell;

impl Language for Haskell {
    fn name(&self) -> &'static str {
        "Haskell"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["hs", "lhs"]
    }
    fn grammar_name(&self) -> &'static str {
        "haskell"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract module name after "import" keyword
        // import qualified Data.Map as M
        let parts: Vec<&str> = text.split_whitespace().collect();
        let mut idx = 1;
        if parts.get(idx) == Some(&"qualified") {
            idx += 1;
        }

        if let Some(module) = parts.get(idx) {
            return vec![Import {
                module: module.to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard: !text.contains('('),
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Haskell: import Module or import Module (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!("import {} ({})", import.module, names_to_use.join(", "))
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/test/**/*.hs", "**/*Spec.hs", "**/*Test.hs"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // tree-sitter-haskell uses "declarations" (not "where") for the body
        node.child_by_field_name("declarations")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // class_declarations / instance_declarations contain declarations
        // directly, with no enclosing keywords in the node itself
        crate::body::analyze_end_body(body_node, content, inner_indent)
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
            "associated_type", "class_declarations", "constructor",
            "constructor_operator", "constructor_synonym", "constructor_synonyms",
            "data_constructor", "data_constructors", "declarations",
            "default_types", "do_module", "explicit_type", "export", "exports",
            "forall", "forall_required", "foreign_export", "foreign_import",
            "function_head_parens", "gadt_constructor", "gadt_constructors",
            "generator", "import_list", "import_name", "import_package", "imports",
            "instance_declarations", "lambda_case", "lambda_cases",
            "linear_function", "list_comprehension", "modifier", "module",
            "module_export", "module_id", "multi_way_if", "newtype_constructor",
            "operator", "qualified", "qualifiers", "quantified_variables",
            "quasiquote_body", "quoted_expression", "quoted_type", "transform",
            "type_application", "type_binder", "type_family",
            "type_family_injectivity", "type_family_result", "type_instance",
            "type_params", "type_patterns", "type_role",
            "typed_quote",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "lambda",
            "case",
            "match",
            "import",
        ];
        validate_unused_kinds_audit(&Haskell, documented_unused)
            .expect("Haskell unused node kinds audit failed");
    }
}
