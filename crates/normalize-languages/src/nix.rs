//! Nix language support.

use std::path::Path;

use crate::{
    Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver, Resolution,
    ResolverConfig,
};
use tree_sitter::Node;

/// Nix language support.
pub struct Nix;

impl Language for Nix {
    fn name(&self) -> &'static str {
        "Nix"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["nix"]
    }
    fn grammar_name(&self) -> &'static str {
        "nix"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "apply_expression" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("import ") {
            return Vec::new();
        }

        // Extract path after "import"
        let rest = text.strip_prefix("import ").unwrap_or("").trim();
        let module = rest.split_whitespace().next().unwrap_or(rest).to_string();

        vec![Import {
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: rest.starts_with('.'),
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Nix: import ./path.nix
        format!("import {}", import.module)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("attrpath")
            .map(|n| &content[n.byte_range()])
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: NixModuleResolver = NixModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Nix {}

// =============================================================================
// Nix Module Resolver
// =============================================================================

/// Module resolver for Nix.
///
/// Nix has no module system — `import` is a file path operation. Relative paths
/// (`./foo.nix`, `../lib/default.nix`) are resolved against the caller's directory.
/// Channel/nixpkgs imports (`<nixpkgs>`) are returned as `NotFound` because they
/// require nix evaluation to resolve.
pub struct NixModuleResolver;

impl ModuleResolver for NixModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: Vec::new(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "nix" {
            return Vec::new();
        }

        let rel = file.strip_prefix(&cfg.workspace_root).unwrap_or(file);
        let path_str = rel.to_string_lossy().into_owned();
        if path_str.is_empty() {
            return Vec::new();
        }
        vec![ModuleId {
            canonical_path: path_str,
        }]
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "nix" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Channel form: <nixpkgs>, <nixpkgs/pkgs/...> — not resolvable
        if raw.starts_with('<') {
            return Resolution::NotFound;
        }

        // Relative paths: ./foo.nix or ../lib/default.nix
        if raw.starts_with("./") || raw.starts_with("../") {
            let base_dir = from_file.parent().unwrap_or(&cfg.workspace_root);
            let candidate = base_dir.join(raw);
            if candidate.exists() {
                return Resolution::Resolved(candidate, String::new());
            }
            return Resolution::NotFound;
        }

        // Bare name (e.g. `nixpkgs`) — not resolvable without nix toolchain
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
            "assert_expression", "binary_expression", "float_expression",
            "formal", "formals", "has_attr_expression", "hpath_expression",
            "identifier", "indented_string_expression", "integer_expression",
            "list_expression", "let_attrset_expression", "parenthesized_expression",
            "path_expression", "select_expression", "spath_expression",
            "string_expression", "unary_expression", "uri_expression",
            "variable_expression",
            // Control flow / application — not definition constructs
            "apply_expression", "if_expression", "with_expression",
            // structural node, not extracted as symbols
            "let_expression",
            "attrset_expression",
            "rec_attrset_expression",
            "function_expression",
        ];
        validate_unused_kinds_audit(&Nix, documented_unused)
            .expect("Nix unused node kinds audit failed");
    }
}
