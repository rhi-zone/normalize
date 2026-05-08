//! R language support.

use std::path::Path;

use crate::docstring::extract_preceding_prefix_comments;
use crate::{
    Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver, Resolution,
    ResolverConfig, Visibility,
};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
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

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // roxygen2 comments start with #'
        extract_preceding_prefix_comments(node, content, "#'")
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: RModuleResolver = RModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for R {}

// =============================================================================
// R Module Resolver
// =============================================================================

/// Module resolver for R.
///
/// `source("./utils.R")` is a relative file load — resolved against the caller's
/// directory. `library(pkg)` / `require(pkg)` are package calls — `NotFound`
/// because they require the R package library to resolve.
pub struct RModuleResolver;

impl ModuleResolver for RModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "R" && ext != "r" {
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
        if ext != "R" && ext != "r" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Relative paths: ./utils.R or ../shared/helpers.R
        if raw.starts_with("./") || raw.starts_with("../") {
            let base_dir = from_file.parent().unwrap_or(&cfg.workspace_root);
            let candidate = base_dir.join(raw);
            if candidate.exists() {
                return Resolution::Resolved(candidate, String::new());
            }
            // Try adding .R extension if no extension present
            if candidate.extension().is_none() {
                let mut with_ext = candidate.clone();
                with_ext.set_extension("R");
                if with_ext.exists() {
                    return Resolution::Resolved(with_ext, String::new());
                }
            }
            return Resolution::NotFound;
        }

        // library(pkg) / require(pkg) — package calls, not resolvable here
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
            "extract_operator", "identifier",
            "namespace_operator", "parenthesized_expression", "return", "unary_operator",
            // control flow — not extracted as symbols
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
