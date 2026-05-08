//! Clojure language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
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

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: ClojureModuleResolver = ClojureModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Clojure {}

// =============================================================================
// Clojure Module Resolver
// =============================================================================

/// Module resolver for Clojure.
///
/// `(ns myapp.core (:require [myapp.utils :as u]))` → `raw = "myapp.utils"`.
/// `myapp.utils` → `myapp/utils.clj` (or `.cljs`, `.cljc`) under `src/` or `test/`.
pub struct ClojureModuleResolver;

impl ModuleResolver for ClojureModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.join("src"), root.join("test"), root.to_path_buf()],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "clj" && ext != "cljs" && ext != "cljc" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let module = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".cljc")
                    .trim_end_matches(".cljs")
                    .trim_end_matches(".clj")
                    .replace('/', ".")
                    .replace('_', "-"); // Clojure: my_utils.clj → my-utils
                if !module.is_empty() {
                    return vec![ModuleId {
                        canonical_path: module,
                    }];
                }
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "clj" && ext != "cljs" && ext != "cljc" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // Skip generic form names
        if raw == "require" || raw == "use" || raw == "import" || raw == "ns" {
            return Resolution::NotFound;
        }
        // Convert namespace to path: myapp.utils → myapp/utils.clj
        let path_part = raw.replace('.', "/").replace('-', "_");
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();

        for search_root in &cfg.search_roots {
            for ext_try in &["clj", "cljs", "cljc"] {
                let candidate = search_root.join(format!("{}.{}", path_part, ext_try));
                if candidate.exists() {
                    return Resolution::Resolved(candidate, exported_name);
                }
            }
        }
        Resolution::NotFound
    }
}

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
