//! Erlang language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Erlang language support.
pub struct Erlang;

impl Language for Erlang {
    fn name(&self) -> &'static str {
        "Erlang"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["erl", "hrl"]
    }
    fn grammar_name(&self) -> &'static str {
        "erlang"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "module_attribute" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Handle -import(module, [...]).
        if text.starts_with("-import(")
            && let Some(start) = text.find('(')
        {
            let rest = &text[start + 1..];
            if let Some(comma) = rest.find(',') {
                let module = rest[..comma].trim().to_string();
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

        // Handle -include("file.hrl"). or -include_lib("app/include/file.hrl").
        if text.starts_with("-include")
            && let Some(start) = text.find('"')
        {
            let rest = &text[start + 1..];
            if let Some(end) = rest.find('"') {
                let module = rest[..end].to_string();
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: text.starts_with("-include("),
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Erlang: -import(module, [func/arity, ...]).
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("-import({}, []).", import.module)
        } else {
            format!("-import({}, [{}]).", import.module, names_to_use.join(", "))
        }
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "module_attribute" {
                let text = content[sibling.byte_range()].trim();
                if text.starts_with("-spec(")
                    || text.starts_with("-spec ")
                    || text.starts_with("-callback(")
                    || text.starts_with("-deprecated(")
                    || text.starts_with("-deprecated.")
                {
                    attrs.insert(0, text.to_string());
                }
                prev = sibling.prev_sibling();
            } else if sibling.kind() == "comment" {
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }
        attrs
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if node.kind() == "function_clause"
            && let Some(name_node) = node.child_by_field_name("name")
        {
            let name = &content[name_node.byte_range()];
            let arity = node
                .child_by_field_name("arguments")
                .map(|args| {
                    let mut cursor = args.walk();
                    args.children(&mut cursor).count()
                })
                .unwrap_or(0);
            return format!("{}/{}", name, arity);
        }
        let text = &content[node.byte_range()];
        text.lines().next().unwrap_or(text).trim().to_string()
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
        &["**/*_SUITE.erl", "**/*_test.erl", "**/*_tests.erl"]
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
            "ann_type", "b_generator", "binary_comprehension", "bit_type_list",
            "bit_type_unit", "block_expr", "catch_expr", "clause_body",
            "cond_match_expr", "deprecated_module", "export_attribute",
            "export_type_attribute", "field_type", "fun_type", "fun_type_sig",
            "generator", "guard_clause", "import_attribute", "list_comprehension",
            "map_comprehension", "map_generator", "match_expr", "module",
            "pp_elif", "pp_else", "pp_endif", "pp_if", "pp_ifdef", "pp_ifndef",
            "range_type", "remote_module", "replacement_cr_clauses",
            "replacement_function_clauses", "ssr_definition", "try_after",
            "try_class", "try_stack", "type_guards", "type_name", "type_sig",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "cr_clause",
            "try_expr",
            "fun_clause",
            "if_expr",
            "if_clause",
            "case_expr",
            "catch_clause",
        ];
        validate_unused_kinds_audit(&Erlang, documented_unused)
            .expect("Erlang unused node kinds audit failed");
    }
}
