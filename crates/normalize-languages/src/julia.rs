//! Julia language support.

use std::path::{Path, PathBuf};

use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig,
};
use tree_sitter::Node;

/// Julia language support.
pub struct Julia;

impl Language for Julia {
    fn name(&self) -> &'static str {
        "Julia"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jl"]
    }
    fn grammar_name(&self) -> &'static str {
        "julia"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // module_definition has a "name" field
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        // function_definition/macro_definition: name in signature (no named children)
        // struct_definition/abstract_definition: name in type_head
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "signature" || child.kind() == "type_head" {
                let text = &content[child.byte_range()];
                // "add(a, b)" → "add", "Foo <: Bar" → "Foo"
                let end = text
                    .find(|c: char| c == '(' || c == '<' || c == '{' || c.is_whitespace())
                    .unwrap_or(text.len());
                if end > 0 {
                    return Some(&content[child.start_byte()..child.start_byte() + end]);
                }
            }
            if child.kind() == "identifier" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let prev = node.prev_sibling()?;
        if prev.kind() != "string_literal" {
            return None;
        }

        let text = &content[prev.byte_range()];
        if !text.starts_with("\"\"\"") {
            return None;
        }

        // Strip the triple quotes and clean up
        let inner = text
            .strip_prefix("\"\"\"")
            .unwrap_or(text)
            .strip_suffix("\"\"\"")
            .unwrap_or(text);

        let lines: Vec<&str> = inner
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            return None;
        }

        Some(lines.join(" "))
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        let (keyword, is_wildcard) = if text.starts_with("using ") {
            ("using ", true)
        } else if text.starts_with("import ") {
            ("import ", false)
        } else {
            return Vec::new();
        };

        let rest = text.strip_prefix(keyword).unwrap_or("");
        let module = rest
            .split([':', ','])
            .next()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if module.is_empty() {
            return Vec::new();
        }

        vec![Import {
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard,
            is_relative: false,
            line,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Julia: using Module or import Module: a, b, c
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("using {}", import.module)
        } else {
            format!("import {}: {}", import.module, names_to_use.join(", "))
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
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: JuliaModuleResolver = JuliaModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Julia {}

// =============================================================================
// Julia Module Resolver
// =============================================================================

/// Module resolver for Julia.
///
/// `include("utils.jl")` is a relative file inclusion resolved against the
/// caller's directory. `using`/`import` of a package name is matched against
/// workspace packages (from `Project.toml`). Other external packages return
/// `NotFound`.
pub struct JuliaModuleResolver;

impl ModuleResolver for JuliaModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let project_toml = root.join("Project.toml");
        if let Ok(content) = std::fs::read_to_string(&project_toml)
            && let Ok(parsed) = content.parse::<toml::Value>()
            && let Some(name) = parsed.get("name").and_then(|v| v.as_str())
        {
            let src_dir = root.join("src");
            path_mappings.push((name.to_string(), src_dir));
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: Vec::new(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "jl" {
            return Vec::new();
        }

        // Try stripping from src/ first, then workspace root
        let src_dir = cfg.workspace_root.join("src");
        let base = if let Ok(rel) = file.strip_prefix(&src_dir) {
            rel.to_string_lossy().into_owned()
        } else if let Ok(rel) = file.strip_prefix(&cfg.workspace_root) {
            rel.to_string_lossy().into_owned()
        } else {
            return Vec::new();
        };

        let canonical = base.strip_suffix(".jl").unwrap_or(&base).to_string();

        if canonical.is_empty() {
            return Vec::new();
        }
        vec![ModuleId {
            canonical_path: canonical,
        }]
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "jl" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // include("file.jl") — relative path, ends with .jl or is_relative
        if spec.is_relative || raw.ends_with(".jl") {
            let base_dir = from_file.parent().unwrap_or(&cfg.workspace_root);
            let candidate = base_dir.join(raw);
            if candidate.exists() {
                return Resolution::Resolved(candidate, String::new());
            }
            // Try adding .jl if not present
            if !raw.ends_with(".jl") {
                let with_ext = base_dir.join(format!("{}.jl", raw));
                if with_ext.exists() {
                    return Resolution::Resolved(with_ext, String::new());
                }
            }
            return Resolution::NotFound;
        }

        // using/import PackageName — check workspace packages
        for (pkg_name, pkg_src) in &cfg.path_mappings {
            if raw == pkg_name {
                // Try src/<PkgName>.jl
                let main_file = pkg_src.join(format!("{}.jl", pkg_name));
                if main_file.exists() {
                    return Resolution::Resolved(main_file, String::new());
                }
                // Fallback: src/ directory itself
                if pkg_src.exists() {
                    return Resolution::Resolved(pkg_src.clone(), String::new());
                }
            }
        }

        // External package — not resolvable
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
            "adjoint_expression", "binary_expression", "block",
            "block_comment", "break_statement", "broadcast_call_expression", "call_expression",
            "catch_clause", "compound_assignment_expression", "compound_statement",
            "comprehension_expression", "continue_statement", "curly_expression", "else_clause",
            "export_statement", "field_expression", "finally_clause", "for_binding", "for_clause",
            "generator", "global_statement", "identifier", "if_clause", "import_alias",
            "import_path", "index_expression", "interpolation_expression",
            "juxtaposition_expression", "local_statement", "macro_identifier",
            "macrocall_expression", "matrix_expression", "operator", "parametrized_type_expression",
            "parenthesized_expression", "public_statement", "quote_expression", "quote_statement",
            "range_expression", "return_statement", "selected_import", "splat_expression",
            "tuple_expression", "typed_expression", "unary_expression",
            "unary_typed_expression", "vector_expression", "where_expression",
            // covered by tags.scm
            "const_statement",
            "arrow_function_expression",
            "if_statement",
            "using_statement",
            "primitive_definition",
            "for_statement",
            "let_statement",
            "ternary_expression",
            "do_clause",
            "while_statement",
            "try_statement",
            "elseif_clause",
            "import_statement",
        ];
        validate_unused_kinds_audit(&Julia, documented_unused)
            .expect("Julia unused node kinds audit failed");
    }
}
