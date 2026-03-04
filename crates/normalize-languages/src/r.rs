//! R language support.

use crate::{Import, Language, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

/// R language support.
pub struct R;

impl Language for R {
    fn name(&self) -> &'static str {
        "R"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["r", "R", "rmd", "Rmd"]
    }
    fn grammar_name(&self) -> &'static str {
        "r"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        // R functions are typically assigned: name <- function(...) {}
        // We need to look at the parent assignment (binary_operator in R grammar)
        let parent = node.parent()?;
        if parent.kind() != "binary_operator" {
            return None;
        }

        let name = parent
            .child(0)
            .map(|n| content[n.byte_range()].to_string())?;
        let text = &content[parent.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: None,
            attributes: Vec::new(),
            start_line: parent.start_position().row + 1,
            end_line: parent.end_position().row + 1,
            visibility: if name.starts_with('.') {
                Visibility::Private
            } else {
                Visibility::Public
            },
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "call" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("library(") && !text.starts_with("require(") {
            return Vec::new();
        }

        // Extract package name from library(pkg) or require(pkg)
        let inner = text
            .split('(')
            .nth(1)
            .and_then(|s| s.split(')').next())
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string());

        if let Some(module) = inner {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: true,
                is_relative: false,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // R: library(package)
        format!("library({})", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if node
            .child(0)
            .is_none_or(|n| !content[n.byte_range()].starts_with('.'))
        {
            Visibility::Public
        } else {
            Visibility::Private
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
        &["**/test-*.R", "**/test_*.R"]
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
            "extract_operator", "identifier",
            "namespace_operator", "parenthesized_expression", "return", "unary_operator",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "braced_expression",
            "if_statement",
            "while_statement",
            "function_definition",
            "repeat_statement",
            "for_statement",
        ];
        validate_unused_kinds_audit(&R, documented_unused)
            .expect("R unused node kinds audit failed");
    }
}
