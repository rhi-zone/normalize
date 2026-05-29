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

    fn as_refactor_codegen(&self) -> Option<&dyn crate::RefactorCodeGen> {
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

impl crate::RefactorCodeGen for Zig {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        match ty {
            Some(t) => format!("{}: {}", name, t),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}const {} = {};\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{}: {}", p.name, ty),
                None => format!("{}: anytype", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let ret_type = match &spec.ret {
            GenReturn::Unit => "void".to_string(),
            GenReturn::Single(v) => format!("/* {} */", v),
            GenReturn::Tuple(vs) => format!("/* ({}) */", vs.join(", ")),
            GenReturn::Result(ok, _) => format!("!/* {} */", ok),
        };
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {};", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    return .{{ {} }};", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => format!("\n{}    return {};", indent, ok),
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}fn {}({}) {} {{\n{}{}\n{}}}\n",
            indent, spec.name, param_str, ret_type, body, return_stmt, indent
        )
    }

    fn render_call_site(&self, spec: &crate::CallSiteSpec) -> String {
        use crate::GenReturn;
        let args = spec
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let name = &spec.name;
        match &spec.ret {
            GenReturn::Unit => format!("{}{}({});\n", indent, name, args),
            GenReturn::Single(v) => format!("{}const {} = {}({});\n", indent, v, name, args),
            GenReturn::Tuple(vs) => format!(
                "{}const .{{ {} }} = {}({});\n",
                indent,
                vs.join(", "),
                name,
                args
            ),
            GenReturn::Result(ok, _) => {
                format!("{}const {} = try {}({});\n", indent, ok, name, args)
            }
        }
    }

    fn uses_result_for_exceptions(&self) -> bool {
        // Zig error unions (`!T`) are the propagated fallible-return type.
        true
    }

    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        // Zig parameters: `name: Type`.
        let pattern = format!("{}: ", name);
        let pos = content.find(&pattern)?;
        let after = &content[pos + pattern.len()..];
        let end = after.find([',', ')', '\n']).unwrap_or(after.len());
        let ty = after[..end].trim().to_string();
        if ty.is_empty() { None } else { Some(ty) }
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Zig;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn zig_fn_basic() {
        let spec = ExtractedFnSpec {
            name: "double".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: Some("i32".to_string()),
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            is_generator: false,
            body_lines: vec!["const result = n * 2;".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            Zig.render_function(&spec),
            "\nfn double(n: i32) /* result */ {\n    const result = n * 2;\n    return result;\n}\n"
        );
    }

    #[test]
    fn zig_fn_void() {
        let spec = ExtractedFnSpec {
            name: "log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["std.debug.print(\"x\", .{});".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            Zig.render_function(&spec),
            "\nfn log() void {\n    std.debug.print(\"x\", .{});\n}\n"
        );
    }

    #[test]
    fn zig_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "double".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: None,
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(
            Zig.render_call_site(&spec),
            "    const result = double(n);\n"
        );
        assert_eq!(Zig.render_binding("x", "f()", "  "), "  const x = f();\n");
        assert_eq!(Zig.format_param("n", Some("i32")), "n: i32");
    }
}

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
