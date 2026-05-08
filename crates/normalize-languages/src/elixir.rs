//! Elixir language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Elixir language support.
pub struct Elixir;

impl Language for Elixir {
    fn name(&self) -> &'static str {
        "Elixir"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }
    fn grammar_name(&self) -> &'static str {
        "elixir"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " end"
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "unary_operator" {
                let text = content[sibling.byte_range()].trim();
                if text.starts_with('@')
                    && !text.starts_with("@doc")
                    && !text.starts_with("@moduledoc")
                {
                    attrs.insert(0, text.to_string());
                }
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }
        attrs
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        if node.kind() != "call" {
            let text = &content[node.byte_range()];
            return text.lines().next().unwrap_or(text).trim().to_string();
        }
        let text = &content[node.byte_range()];
        if text.starts_with("defmodule ")
            && let Some(name) = self.extract_module_name(node, content)
        {
            return format!("defmodule {}", name);
        }
        // For def/defp/defmacro: take first line, trim trailing " do"
        let first_line = text.lines().next().unwrap_or(text).trim();
        first_line.trim_end_matches(" do").to_string()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "call" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Handle import, alias, require, use
        for keyword in &["import ", "alias ", "require ", "use "] {
            if let Some(stripped) = text.strip_prefix(keyword) {
                let rest = stripped.trim();
                let module = rest
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .next()
                    .unwrap_or(rest)
                    .to_string();

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
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Elixir: import Module or import Module, only: [a: 1, b: 2]
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!(
                "import {}, only: [{}]",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if node.kind() != "call" {
            return Visibility::Private;
        }
        let text = &content[node.byte_range()];
        let is_public = (text.starts_with("def ") && !text.starts_with("defp"))
            || (text.starts_with("defmacro ") && !text.starts_with("defmacrop"))
            || text.starts_with("defmodule ");
        if is_public {
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
        &["**/test/**/*.exs", "**/*_test.exs"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Look for do_block child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "do_block")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_do_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if node.kind() != "call" {
            // Fall back to default (child_by_field_name("name"))
            return node
                .child_by_field_name("name")
                .map(|n| &content[n.byte_range()]);
        }
        // For Elixir call nodes (def/defp/defmodule/defmacro/defprotocol/defimpl):
        // - defmodule MathUtils → arguments > alias
        // - def add(a, b) → arguments > call > target > identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                let mut arg_cursor = child.walk();
                for arg in child.children(&mut arg_cursor) {
                    match arg.kind() {
                        // defmodule/defprotocol/defimpl: (arguments (alias) ...)
                        "alias" => return Some(&content[arg.byte_range()]),
                        // def/defp/defmacro: (arguments (call target: (identifier) ...) ...)
                        "call" => {
                            if let Some(target) = arg.child_by_field_name("target") {
                                return Some(&content[target.byte_range()]);
                            }
                        }
                        // def with no args: (arguments (identifier) ...)
                        "identifier" => return Some(&content[arg.byte_range()]),
                        _ => {}
                    }
                }
            }
        }
        None
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: ElixirModuleResolver = ElixirModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Elixir {}

// =============================================================================
// Elixir Module Resolver
// =============================================================================

/// Module resolver for Elixir (Mix conventions).
///
/// Mix convention: `lib/my_app/utils.ex` contains module `MyApp.Utils`.
/// Converts CamelCase module name ↔ snake_case path.
pub struct ElixirModuleResolver;

/// Convert a CamelCase module name component to snake_case.
fn camel_to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_lowercase().next().unwrap_or(c));
    }
    out
}

/// Convert a snake_case path segment to CamelCase.
fn snake_to_camel(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

impl ModuleResolver for ElixirModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let mix_exs = root.join("mix.exs");
        if let Ok(content) = std::fs::read_to_string(&mix_exs) {
            // Parse `app: :my_app`
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("app:") {
                    let rest = rest.trim();
                    let app_atom = rest
                        .trim_start_matches(':')
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !app_atom.is_empty() {
                        // Convert my_app → MyApp for the module prefix
                        let module_prefix = snake_to_camel(app_atom);
                        path_mappings.push((module_prefix, root.join("lib")));
                        break;
                    }
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: vec![root.join("lib"), root.join("test")],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "ex" && ext != "exs" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let module_path: String = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".exs")
                    .trim_end_matches(".ex")
                    .split('/')
                    .map(snake_to_camel)
                    .collect::<Vec<_>>()
                    .join(".");
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
        if ext != "ex" && ext != "exs" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // Convert MyApp.Utils → my_app/utils.ex
        let path_part = raw
            .split('.')
            .map(camel_to_snake)
            .collect::<Vec<_>>()
            .join("/");
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();

        for search_root in &cfg.search_roots {
            for ext_try in &["ex", "exs"] {
                let candidate = search_root.join(format!("{}.{}", path_part, ext_try));
                if candidate.exists() {
                    return Resolution::Resolved(candidate, exported_name);
                }
            }
        }
        Resolution::NotFound
    }
}

impl Elixir {
    fn extract_module_name(&self, node: &Node, content: &str) -> Option<String> {
        // Look for the module name after defmodule
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "alias" || child.kind() == "atom" {
                let text = &content[child.byte_range()];
                if !text.is_empty() && text != "defmodule" {
                    return Some(text.to_string());
                }
            }
        }
        None
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
            "after_block", "block", "body", "catch_block", "charlist",
            "else_block", "interpolation", "operator_identifier",
            "rescue_block", "sigil_modifiers", "stab_clause", "struct",
            "unary_operator",
            // control flow — not extracted as symbols
            "binary_operator",
            "do_block",
            "anonymous_function",
        ];
        validate_unused_kinds_audit(&Elixir, documented_unused)
            .expect("Elixir unused node kinds audit failed");
    }
}
