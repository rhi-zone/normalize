//! Elm language support.

use std::path::{Path, PathBuf};

use crate::{
    Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver, Resolution,
    ResolverConfig,
};
use tree_sitter::Node;

/// Elm language support.
pub struct Elm;

impl Language for Elm {
    fn name(&self) -> &'static str {
        "Elm"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["elm"]
    }
    fn grammar_name(&self) -> &'static str {
        "elm"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // value_declaration: name is in function_declaration_left > lower_case_identifier
        if node.kind() == "value_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "function_declaration_left" {
                    let mut inner = child.walk();
                    for grandchild in child.children(&mut inner) {
                        if grandchild.kind() == "lower_case_identifier" {
                            return Some(&content[grandchild.byte_range()]);
                        }
                    }
                }
            }
            return None;
        }
        // type_alias_declaration, type_declaration: direct upper_case_identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "upper_case_identifier" || child.kind() == "lower_case_identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_clause" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import Module.Name [as Alias] [exposing (..)]
        if let Some(rest) = text.strip_prefix("import ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(&module) = parts.first() {
                let alias = parts
                    .iter()
                    .position(|&p| p == "as")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());

                return vec![Import {
                    module: module.to_string(),
                    names: Vec::new(),
                    alias,
                    is_wildcard: text.contains("exposing (..)"),
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Elm: import Module or import Module exposing (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {} exposing (..)", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!(
                "import {} exposing ({})",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let prev = node.prev_sibling()?;

        if prev.kind() != "block_comment" {
            return None;
        }

        let text = &content[prev.byte_range()];
        // Elm doc comments start with {-| and end with -}
        let inner = text.strip_prefix("{-|")?;
        let inner = inner.strip_suffix("-}").unwrap_or(inner).trim().to_string();
        if inner.is_empty() { None } else { Some(inner) }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: ElmModuleResolver = ElmModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Elm {}

// =============================================================================
// Elm Module Resolver
// =============================================================================

/// Module resolver for Elm.
///
/// Reads `elm.json` to find source directories. Module names map directly
/// to file paths: `Html.Attributes` → `Html/Attributes.elm` under a source root.
pub struct ElmModuleResolver;

impl ModuleResolver for ElmModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut search_roots: Vec<PathBuf> = Vec::new();

        let elm_json = root.join("elm.json");
        if let Ok(content) = std::fs::read_to_string(&elm_json)
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(dirs) = parsed.get("source-directories").and_then(|v| v.as_array())
        {
            for dir in dirs {
                if let Some(s) = dir.as_str() {
                    search_roots.push(root.join(s));
                }
            }
        }

        // Default to src/ if elm.json not found or has no source-directories
        if search_roots.is_empty() {
            search_roots.push(root.join("src"));
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots,
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "elm" {
            return Vec::new();
        }

        for root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(root) {
                let rel_str = rel.to_string_lossy();
                // Strip .elm and replace / with .
                let base = rel_str.strip_suffix(".elm").unwrap_or(&rel_str);
                let canonical = if cfg!(windows) {
                    base.replace('\\', ".")
                } else {
                    base.replace('/', ".")
                };
                if !canonical.is_empty() {
                    return vec![ModuleId {
                        canonical_path: canonical,
                    }];
                }
            }
        }

        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "elm" {
            return Resolution::NotApplicable;
        }

        // `Html.Attributes` → `Html/Attributes.elm`
        let file_path = spec.raw.replace('.', "/") + ".elm";

        for root in &cfg.search_roots {
            let candidate = root.join(&file_path);
            if candidate.exists() {
                return Resolution::Resolved(candidate, String::new());
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
            "as_clause", "block_comment", "case", "exposed_operator", "exposed_type",
            "exposed_union_constructors", "field_accessor_function_expr", "field_type",
            "function_call_expr", "import", "infix_declaration",
            "lower_type_name", "module", "nullary_constructor_argument_pattern",
            "operator", "operator_as_function_expr", "operator_identifier",
            "record_base_identifier", "record_type", "tuple_type", "type",
            "type_annotation", "type_expression", "type_ref", "type_variable",
            "upper_case_qid",
            // control flow — not extracted as symbols
            "if_else_expr",
            "import_clause",
            "anonymous_function_expr",
            "module_declaration",
            "case_of_expr",
            "case_of_branch",
        ];
        validate_unused_kinds_audit(&Elm, documented_unused)
            .expect("Elm unused node kinds audit failed");
    }
}
