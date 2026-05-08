//! Gleam language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Gleam language support.
pub struct Gleam;

impl Language for Gleam {
    fn name(&self) -> &'static str {
        "Gleam"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["gleam"]
    }
    fn grammar_name(&self) -> &'static str {
        "gleam"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import module/path
        if let Some(rest) = text.strip_prefix("import ") {
            let module = rest.split_whitespace().next().unwrap_or("").to_string();

            if !module.is_empty() {
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

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Gleam: import module or import module.{a, b, c}
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!("import {}.{{{}}}", import.module, names_to_use.join(", "))
        }
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            let kind = sibling.kind();
            if kind == "comment" || kind == "statement_comment" {
                let text = &content[sibling.byte_range()];
                // Doc comments start with ///
                if let Some(line) = text.strip_prefix("///") {
                    let line = line.strip_prefix(' ').unwrap_or(line);
                    doc_lines.push(line.to_string());
                } else {
                    break;
                }
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        let joined = doc_lines.join("\n").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if content[node.byte_range()].starts_with("pub ") {
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

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: GleamModuleResolver = GleamModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Gleam {}

// =============================================================================
// Gleam Module Resolver
// =============================================================================

/// Module resolver for Gleam.
///
/// Reads `gleam.toml` for the package name. Resolves:
/// - `import myapp/utils` → `src/utils.gleam` (or subdirectory path)
/// - Standard library imports (`gleam/list`, etc.) → `NotFound`
pub struct GleamModuleResolver;

impl ModuleResolver for GleamModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let gleam_toml = root.join("gleam.toml");
        if let Ok(content) = std::fs::read_to_string(&gleam_toml) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("name") {
                    let rest = rest.trim_start_matches([' ', '=']).trim();
                    let name = rest.trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        path_mappings.push((name.to_string(), root.join("src")));
                        break;
                    }
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: vec![root.join("src"), root.join("test")],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "gleam" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let module = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".gleam")
                    .replace('\\', "/");
                if !module.is_empty() {
                    // Return as pkg/module path: myapp/utils
                    if let Some((pkg, _)) = cfg.path_mappings.first() {
                        return vec![ModuleId {
                            canonical_path: format!("{}/{}", pkg, module),
                        }];
                    }
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
        if ext != "gleam" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // e.g. "myapp/utils" or "gleam/list"
        // Split on first slash: first segment is the package name
        let slash = raw.find('/');
        let (pkg, path_in_pkg) = if let Some(idx) = slash {
            (&raw[..idx], &raw[idx + 1..])
        } else {
            (raw.as_str(), "")
        };

        let exported_name = raw.rsplit('/').next().unwrap_or(raw).to_string();

        // Check if it's our own package
        for (own_pkg, src_dir) in &cfg.path_mappings {
            if pkg == own_pkg {
                let file_path = if path_in_pkg.is_empty() {
                    format!("{}.gleam", pkg)
                } else {
                    format!("{}.gleam", path_in_pkg)
                };
                let candidate = src_dir.join(&file_path);
                if candidate.exists() {
                    return Resolution::Resolved(candidate, exported_name);
                }
                return Resolution::NotFound;
            }
        }

        // Standard library and external packages → NotFound
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
            // Type-related nodes
            "data_constructor", "data_constructor_argument", "data_constructor_arguments",
            "data_constructors", "external_type", "function_parameter", "function_parameter_types",
            "function_parameters", "function_type", "opacity_modifier", "remote_type_identifier",
            "tuple_type", "type", "type_argument", "type_arguments", "type_hole", "type_identifier",
            "type_name", "type_parameter", "type_parameters", "type_var", "visibility_modifier",
            // Case clause patterns
            "case_clause_guard", "case_clause_pattern", "case_clause_patterns", "case_clauses",
            "case_subjects",
            // Function-related nodes
            "binary_expression", "constructor_name", "external_function", "external_function_body",
            "function_call", "remote_constructor_name",
            // Import-related nodes
            "unqualified_import", "unqualified_imports",
            // Comments and identifiers
            "identifier", "module", "module_comment", "statement_comment",
            // structural node, not extracted as symbols
            "block",
            "import",
            "anonymous_function",
            "case_clause",
            "case",
        ];
        validate_unused_kinds_audit(&Gleam, documented_unused)
            .expect("Gleam unused node kinds audit failed");
    }
}
