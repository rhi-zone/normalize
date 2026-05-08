//! ReScript language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// ReScript language support.
pub struct ReScript;

impl Language for ReScript {
    fn name(&self) -> &'static str {
        "ReScript"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["res", "resi"]
    }
    fn grammar_name(&self) -> &'static str {
        "rescript"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "open_statement" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: true,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // ReScript: open Module
        format!("open {}", import.module)
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
        static RESOLVER: ReScriptModuleResolver = ReScriptModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for ReScript {}

// =============================================================================
// ReScript Module Resolver
// =============================================================================

/// Module resolver for ReScript (BuckleScript/rescript-lang conventions).
///
/// ReScript module name = capitalized filename stem.
/// `open MyModule` → `MyModule.res` or `MyModule.resi` in source directories.
pub struct ReScriptModuleResolver;

impl ModuleResolver for ReScriptModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut search_roots: Vec<PathBuf> = Vec::new();

        // Parse bsconfig.json or rescript.json for "sources"
        for config_name in &["bsconfig.json", "rescript.json"] {
            let config_path = root.join(config_name);
            if let Ok(content) = std::fs::read_to_string(&config_path)
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(sources) = json.get("sources")
            {
                match sources {
                    serde_json::Value::String(s) => {
                        let dir = root.join(s);
                        if dir.is_dir() {
                            search_roots.push(dir);
                        }
                    }
                    serde_json::Value::Array(arr) => {
                        for item in arr {
                            let dir_str = item
                                .as_str()
                                .or_else(|| item.get("dir").and_then(|d| d.as_str()));
                            if let Some(dir_s) = dir_str {
                                let dir = root.join(dir_s);
                                if dir.is_dir() {
                                    search_roots.push(dir);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                break;
            }
        }

        if search_roots.is_empty() {
            // Defaults: src/, lib/, root
            for d in &["src", "lib"] {
                let dir = root.join(d);
                if dir.is_dir() {
                    search_roots.push(dir);
                }
            }
            search_roots.push(root.to_path_buf());
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots,
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, _cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "res" && ext != "resi" {
            return Vec::new();
        }
        if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
            // Capitalize first letter for module name
            let module_name = {
                let mut chars = stem.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                }
            };
            return vec![ModuleId {
                canonical_path: module_name,
            }];
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "res" && ext != "resi" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // Strip "open " prefix
        let module_name = raw.strip_prefix("open ").unwrap_or(raw).trim();
        if module_name.is_empty() {
            return Resolution::NotFound;
        }

        for search_root in &cfg.search_roots {
            for ext_try in &["res", "resi"] {
                let candidate = search_root.join(format!("{}.{}", module_name, ext_try));
                if candidate.exists() {
                    return Resolution::Resolved(candidate, module_name.to_string());
                }
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
            // Expression nodes
            "try_expression", "ternary_expression", "while_expression", "for_expression",
            "call_expression", "pipe_expression", "sequence_expression", "await_expression",
            "coercion_expression", "lazy_expression", "assert_expression",
            "parenthesized_expression", "unary_expression", "binary_expression",
            "subscript_expression", "member_expression", "mutation_expression",
            "extension_expression",
            // Type nodes
            "type_identifier", "type_identifier_path", "unit_type", "generic_type",
            "function_type", "polyvar_type", "polymorphic_type", "tuple_type",
            "record_type", "record_type_field", "object_type", "variant_type",
            "abstract_type", "type_arguments", "type_parameters", "type_constraint",
            "type_annotation", "type_spread", "constrain_type",
            "as_aliasing_type", "function_type_parameters",
            // Module nodes
            "parenthesized_module_expression", "module_type_constraint", "module_type_annotation",
            "module_type_of", "constrain_module", "module_identifier", "module_identifier_path",
            "module_pack", "module_unpack",
            // Declaration nodes
            "let_declaration", "exception_declaration", "variant_declaration",
            "polyvar_declaration", "include_statement",
            // JSX
            "jsx_expression", "jsx_identifier", "nested_jsx_identifier",
            // Pattern matching
            "exception_pattern", "polyvar_type_pattern",
            // Identifiers
            "value_identifier_path", "variant_identifier",
            "nested_variant_identifier", "polyvar_identifier", "property_identifier",
            "extension_identifier", "decorator_identifier",
            // Clauses
            "else_clause", "else_if_clause",
            // Other
            "function", "expression_statement", "formal_parameters",
            // control flow — not extracted as symbols
            "if_expression",
            "block",
            "switch_expression",
            "open_statement",
            "switch_match",
        ];
        validate_unused_kinds_audit(&ReScript, documented_unused)
            .expect("ReScript unused node kinds audit failed");
    }
}
