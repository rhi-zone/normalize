//! Clojure language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use tree_sitter::Node;

/// Clojure language support.
pub struct Clojure;

impl Language for Clojure {
    fn name(&self) -> &'static str {
        "Clojure"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["clj", "cljs", "cljc", "edn"]
    }
    fn grammar_name(&self) -> &'static str {
        "clojure"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list_lit" {
            return Vec::new();
        }

        let (form, _) = match self.extract_def_form(node, content) {
            Some(info) => info,
            None => return Vec::new(),
        };

        if form != "require" && form != "use" && form != "import" {
            return Vec::new();
        }

        // Basic extraction - just note the require/import exists
        vec![Import {
            module: form,
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Clojure: (require '[namespace]) or (require '[namespace :refer [a b c]])
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(require '[{}])", import.module)
        } else {
            format!(
                "(require '[{} :refer [{}]])",
                import.module,
                names_to_use.join(" ")
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if let Some((form, _)) = self.extract_def_form(node, content) {
            if form.ends_with('-') {
                Visibility::Private
            } else {
                Visibility::Public
            }
        } else {
            Visibility::Public
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
        &["**/*_test.clj", "**/*_test.cljs", "**/*_test.cljc"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // list_lit is itself "( ... )" — use node directly for paren analysis
        Some(*node)
    }
    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_paren_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // list_lit captured by @definition.* — the name is in the second sym_lit child
        // (first sym_lit is the form: defn, defrecord, ns, etc.)
        if node.kind() != "list_lit" {
            return node
                .child_by_field_name("name")
                .map(|n| &content[n.byte_range()]);
        }
        let mut cursor = node.walk();
        let mut seen_form = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "sym_lit" {
                if !seen_form {
                    seen_form = true;
                } else {
                    return Some(&content[child.byte_range()]);
                }
            }
        }
        None
    }
}

impl LanguageSymbols for Clojure {}

impl Clojure {
    /// Extract the form name and symbol name from a list like (defn foo ...)
    fn extract_def_form(&self, node: &Node, content: &str) -> Option<(String, String)> {
        let mut cursor = node.walk();
        let mut form = None;
        let mut name = None;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "sym_lit" if form.is_none() => {
                    form = Some(content[child.byte_range()].to_string());
                }
                "sym_lit" if form.is_some() && name.is_none() => {
                    name = Some(content[child.byte_range()].to_string());
                    break;
                }
                _ => {}
            }
        }

        Some((form?, name?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[];
        validate_unused_kinds_audit(&Clojure, documented_unused)
            .expect("Clojure unused node kinds audit failed");
    }
}
