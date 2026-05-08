//! Zig language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// Zig language support.
pub struct Zig;

impl Language for Zig {
    fn name(&self) -> &'static str {
        "Zig"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }
    fn grammar_name(&self) -> &'static str {
        "zig"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        // Look for @import("module")
        if node.kind() != "builtin_call_expression" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        if !text.starts_with("@import") {
            return Vec::new();
        }

        // Extract the string argument
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string_literal" {
                let module = content[child.byte_range()].trim_matches('"').to_string();
                let is_relative = module.starts_with('.');
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative,
                    line: node.start_position().row + 1,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Zig: @import("module")
        format!("@import(\"{}\")", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        // Check for pub keyword before the declaration
        if let Some(prev) = node.prev_sibling() {
            let text = &content[prev.byte_range()];
            if text == "pub" {
                return Visibility::Public;
            }
        }
        // Also check if node starts with pub
        let text = &content[node.byte_range()];
        if text.starts_with("pub ") {
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // FnProto uses field "function" for the name identifier.
        // VarDecl uses field "variable_type_function" for the name identifier.
        let name_node = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("variable_type_function"))?;
        Some(&content[name_node.byte_range()])
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: ZigModuleResolver = ZigModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Zig {}

// =============================================================================
// Zig Module Resolver
// =============================================================================

/// Module resolver for Zig.
///
/// Zig uses `@import("path.zig")` for file imports. Relative paths are resolved
/// relative to the importing file. `@import("std")` and other named imports
/// return `NotFound`.
pub struct ZigModuleResolver;

impl ModuleResolver for ZigModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, root: &Path, file: &Path, _cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "zig" {
            return Vec::new();
        }
        if let Ok(rel) = file.strip_prefix(root) {
            let rel_str = rel.to_str().unwrap_or("").replace('\\', "/");
            return vec![ModuleId {
                canonical_path: rel_str,
            }];
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, _cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "zig" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // Named imports (std, builtin, etc.) — not resolvable to files
        if !raw.starts_with('.') && !raw.ends_with(".zig") {
            return Resolution::NotFound;
        }
        // Relative path import
        if let Some(parent) = from_file.parent() {
            let resolved = parent.join(raw);
            if resolved.exists() {
                let name = resolved
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                return Resolution::Resolved(resolved, name);
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
            // Zig grammar uses PascalCase node kinds
            "ArrayTypeStart", "BUILTINIDENTIFIER", "BitShiftOp", "BlockExpr",
            "BlockExprStatement", "BlockLabel", "BuildinTypeExpr", "ContainerDeclType",
            "ForArgumentsList", "ForExpr", "ForItem", "ForPrefix", "ForTypeExpr",
            "FormatSequence", "IDENTIFIER", "IfExpr", "IfPrefix", "IfTypeExpr",
            "LabeledStatement", "LabeledTypeExpr", "LoopExpr", "LoopStatement",
            "LoopTypeExpr", "ParamType", "PrefixTypeOp", "PtrTypeStart",
            "SliceTypeStart", "Statement", "SwitchCase", "WhileContinueExpr",
            "WhileExpr", "WhilePrefix", "WhileTypeExpr",
            // control flow — not extracted as symbols
            "ForStatement",
            "WhileStatement",
            "Block",
            "IfStatement",
        ];
        validate_unused_kinds_audit(&Zig, documented_unused)
            .expect("Zig unused node kinds audit failed");
    }
}
