//! Haskell language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols};
use std::path::{Path, PathBuf};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_haddock(node, content)
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

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        // instance MyClass Foo where → symbol name is "MyClass", implements = ["MyClass"]
        if node.kind() == "instance"
            && let Some(name_node) = node.child_by_field_name("name")
        {
            let class_name = content[name_node.byte_range()].to_string();
            return crate::ImplementsInfo {
                is_interface: false,
                implements: vec![class_name],
            };
        }
        crate::ImplementsInfo::default()
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

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: HaskellModuleResolver = HaskellModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Haskell {}

// =============================================================================
// Haskell Module Resolver
// =============================================================================

/// Module resolver for Haskell (Cabal/Stack conventions).
///
/// `import Data.Map` → `Data/Map.hs` (or `.lhs`) in the source directory.
pub struct HaskellModuleResolver;

impl ModuleResolver for HaskellModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut search_roots: Vec<PathBuf> = Vec::new();

        // Try to find source roots from .cabal file
        let found_cabal = std::fs::read_dir(root).ok().and_then(|entries| {
            entries.flatten().find(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "cabal")
                    .unwrap_or(false)
            })
        });

        if let Some(cabal_entry) = found_cabal
            && let Ok(content) = std::fs::read_to_string(cabal_entry.path())
        {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("hs-source-dirs:") {
                    for dir in rest.split_whitespace() {
                        let candidate = root.join(dir.trim_matches(','));
                        if candidate.is_dir() {
                            search_roots.push(candidate);
                        }
                    }
                }
            }
        }

        if search_roots.is_empty() {
            // Default source roots
            let src = root.join("src");
            if src.is_dir() {
                search_roots.push(src);
            }
            search_roots.push(root.to_path_buf());
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots,
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "hs" && ext != "lhs" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let module_path = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".lhs")
                    .trim_end_matches(".hs")
                    .replace(['/', '\\'], ".");
                if !module_path.is_empty() {
                    return vec![ModuleId {
                        canonical_path: module_path,
                    }];
                }
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "hs" && ext != "lhs" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        let path_part = raw.replace('.', "/");
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();

        for search_root in &cfg.search_roots {
            for ext_try in &["hs", "lhs"] {
                let candidate = search_root.join(format!("{}.{}", path_part, ext_try));
                if candidate.exists() {
                    return Resolution::Resolved(candidate, exported_name.clone());
                }
            }
        }
        Resolution::NotFound
    }
}

/// Extract a Haddock documentation comment preceding a definition node.
///
/// Haddock comments use `-- |` (preceding) or `-- ^` (following) syntax.
/// The tree-sitter-haskell grammar parses these as `haddock` nodes.
///
/// The `haddock` node is a sibling of the `declarations` container, not a
/// sibling of the `function`/`data_type`/etc. inside it. So we walk up to the
/// parent (`declarations`) and check the parent's prev sibling.
fn extract_haddock(node: &Node, content: &str) -> Option<String> {
    // First check immediate prev siblings (within declarations)
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        match sibling.kind() {
            "haddock" => {
                return Some(clean_haddock(&content[sibling.byte_range()]));
            }
            "signature" => {
                // Skip type signature between haddock and function definition
            }
            _ => break,
        }
        prev = sibling.prev_sibling();
    }

    // Check if the parent's prev sibling is a haddock node.
    // This handles the case where haddock is at the top level (sibling of
    // `declarations`) while the definition node is inside `declarations`.
    if let Some(parent) = node.parent()
        && let Some(sibling) = parent.prev_sibling()
        && sibling.kind() == "haddock"
    {
        return Some(clean_haddock(&content[sibling.byte_range()]));
    }

    None
}

/// Clean a Haddock comment into plain text.
///
/// Strips `-- |`, `-- ^`, and `--` prefixes from each line.
fn clean_haddock(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| {
            let l = l.trim();
            if let Some(rest) = l.strip_prefix("-- |") {
                rest.trim()
            } else if let Some(rest) = l.strip_prefix("-- ^") {
                rest.trim()
            } else if let Some(rest) = l.strip_prefix("--") {
                rest.strip_prefix(' ').unwrap_or(rest)
            } else {
                l
            }
        })
        .filter(|l| !l.is_empty())
        .collect();
    lines.join(" ")
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
            // control flow — not extracted as symbols
            "lambda",
            "case",
            "match",
            "import",
        ];
        validate_unused_kinds_audit(&Haskell, documented_unused)
            .expect("Haskell unused node kinds audit failed");
    }
}
