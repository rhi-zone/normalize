//! Scheme language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols};
use std::path::Path;
use tree_sitter::Node;

/// Scheme language support.
pub struct Scheme;

impl Language for Scheme {
    fn name(&self) -> &'static str {
        "Scheme"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scm", "ss", "rkt"]
    }
    fn grammar_name(&self) -> &'static str {
        "scheme"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "list" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        for prefix in &["(import ", "(require "] {
            if text.starts_with(prefix) {
                return vec![Import {
                    module: "import".to_string(),
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Scheme: (import (library)) or (import (only (library) a b c))
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("(import ({}))", import.module)
        } else {
            format!(
                "(import (only ({}) {}))",
                import.module,
                names_to_use.join(" ")
            )
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // list is itself "( ... )" — use node directly for paren analysis
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
        // The @definition.* captures a list for (define ...) forms.
        // Two cases:
        // 1. (define (name args) body) — second named-child is a list; its first
        //    symbol child is the function name.
        // 2. (define name ...) — second named-child is a symbol (the name directly).
        if node.kind() != "list" {
            return node
                .child_by_field_name("name")
                .map(|n| &content[n.byte_range()]);
        }
        let mut cursor = node.walk();
        let mut seen_define = false;
        for child in node.children(&mut cursor) {
            match child.kind() {
                "symbol" if !seen_define => {
                    // First symbol is the form keyword (define, define-syntax, etc.)
                    seen_define = true;
                }
                "symbol" if seen_define => {
                    // Second symbol: (define name ...)
                    return Some(&content[child.byte_range()]);
                }
                "list" if seen_define => {
                    // Second child is a nested list: (define (name args) ...) form
                    // The name is the first symbol inside this nested list.
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "symbol" {
                            return Some(&content[inner.byte_range()]);
                        }
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: SchemeModuleResolver = SchemeModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Scheme {}

// =============================================================================
// Scheme Module Resolver
// =============================================================================

/// Module resolver for Scheme (R7RS library conventions).
///
/// `(import (mylib utils))` → `mylib/utils.sld` or `mylib/utils.scm`.
/// The `ImportSpec.raw` is stored as "import" by the current extractor (too basic),
/// so for now we only resolve when a proper module path is passed.
pub struct SchemeModuleResolver;

impl ModuleResolver for SchemeModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, root: &Path, file: &Path, _cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "scm" && ext != "ss" && ext != "sld" {
            return Vec::new();
        }
        if let Ok(rel) = file.strip_prefix(root) {
            let rel_str = rel.to_str().unwrap_or("");
            let module = rel_str
                .trim_end_matches(".sld")
                .trim_end_matches(".scm")
                .trim_end_matches(".ss")
                .replace('/', " ");
            if !module.is_empty() {
                return vec![ModuleId {
                    canonical_path: format!("({})", module),
                }];
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "scm" && ext != "ss" && ext != "sld" && ext != "rkt" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;

        // The current extractor stores "import" as raw — not resolvable
        if raw == "import" || raw == "require" {
            return Resolution::NotFound;
        }

        // Try to handle R7RS-style: "(mylib utils)" → mylib/utils.sld or .scm
        let normalized = raw.trim_start_matches('(').trim_end_matches(')');
        let path_part = normalized.replace(' ', "/");
        let exported_name = normalized
            .rsplit(' ')
            .next()
            .unwrap_or(normalized)
            .to_string();

        for ext_try in &["sld", "scm", "ss"] {
            let candidate = cfg
                .workspace_root
                .join(format!("{}.{}", path_part, ext_try));
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name.clone());
            }
        }
        Resolution::NotFound
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
            "block_comment",
        ];
        validate_unused_kinds_audit(&Scheme, documented_unused)
            .expect("Scheme unused node kinds audit failed");
    }
}
