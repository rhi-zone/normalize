//! TypeScript language support.

use std::path::{Path, PathBuf};

use crate::ecmascript;
use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// TypeScript language support.
pub struct TypeScript;

/// TSX language support (TypeScript + JSX).
pub struct Tsx;

impl Language for TypeScript {
    fn name(&self) -> &'static str {
        "TypeScript"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "mts", "cts"]
    }
    fn grammar_name(&self) -> &'static str {
        "typescript"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn as_refactor_codegen(&self) -> Option<&dyn crate::RefactorCodeGen> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        ecmascript::extract_jsdoc(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        ecmascript::extract_implements(node, content)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        ecmascript::build_signature(node, content, name)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        ecmascript::format_import(import, names)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                name.starts_with("test_")
                    || name.starts_with("Test")
                    || name == "describe"
                    || name == "it"
                    || name == "test"
            }
            crate::SymbolKind::Module => name == "tests" || name == "test" || name == "__tests__",
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/__tests__/**/*.ts",
            "**/__mocks__/**/*.ts",
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/*.test.tsx",
            "**/*.spec.tsx",
        ]
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        ecmascript::extract_decorators(node, content)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Try 'body' field first, then look for interface_body or class_body child
        if let Some(body) = node.child_by_field_name("body") {
            return Some(body);
        }
        // Fallback: find interface_body or class_body child
        for i in 0..node.child_count() as u32 {
            if let Some(child) = node.child(i)
                && (child.kind() == "interface_body" || child.kind() == "class_body")
            {
                return Some(child);
            }
        }
        None
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        ecmascript::get_visibility(node, content)
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        ecmascript::extract_js_module_doc(src)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: TsModuleResolver = TsModuleResolver;
        Some(&RESOLVER)
    }

    fn post_process_symbols(
        &self,
        symbols: &mut Vec<crate::Symbol>,
        resolver: Option<&dyn crate::InterfaceResolver>,
        current_file: &str,
    ) {
        ecmascript::mark_interface_implementations(symbols, resolver, current_file);
    }
}

impl LanguageSymbols for TypeScript {}

impl crate::RefactorCodeGen for TypeScript {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        ecmascript::refactor_format_param(true, name, ty)
    }
    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        ecmascript::refactor_render_binding(name, expr, indent)
    }
    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        ecmascript::refactor_render_function(true, spec)
    }
    fn render_call_site(&self, spec: &crate::CallSiteSpec) -> String {
        ecmascript::refactor_render_call_site(spec)
    }
    fn supports_multi_return(&self) -> bool {
        true
    }
    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        ecmascript::refactor_infer_param_type(content, name)
    }
}

impl crate::RefactorCodeGen for Tsx {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        ecmascript::refactor_format_param(true, name, ty)
    }
    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        ecmascript::refactor_render_binding(name, expr, indent)
    }
    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        ecmascript::refactor_render_function(true, spec)
    }
    fn render_call_site(&self, spec: &crate::CallSiteSpec) -> String {
        ecmascript::refactor_render_call_site(spec)
    }
    fn supports_multi_return(&self) -> bool {
        true
    }
    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        ecmascript::refactor_infer_param_type(content, name)
    }
}

// TSX shares the same implementation as TypeScript, just with a different grammar
impl Language for Tsx {
    fn name(&self) -> &'static str {
        "TSX"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tsx"]
    }
    fn grammar_name(&self) -> &'static str {
        "tsx"
    }

    fn as_refactor_codegen(&self) -> Option<&dyn crate::RefactorCodeGen> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        ecmascript::extract_jsdoc(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        ecmascript::extract_implements(node, content)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        ecmascript::build_signature(node, content, name)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        ecmascript::format_import(import, names)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                name.starts_with("test_")
                    || name.starts_with("Test")
                    || name == "describe"
                    || name == "it"
                    || name == "test"
            }
            crate::SymbolKind::Module => name == "tests" || name == "test" || name == "__tests__",
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/__tests__/**/*.ts",
            "**/__mocks__/**/*.ts",
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/*.test.tsx",
            "**/*.spec.tsx",
        ]
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        ecmascript::extract_decorators(node, content)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // Try 'body' field first, then look for interface_body or class_body child
        if let Some(body) = node.child_by_field_name("body") {
            return Some(body);
        }
        // Fallback: find interface_body or class_body child
        for i in 0..node.child_count() as u32 {
            if let Some(child) = node.child(i)
                && (child.kind() == "interface_body" || child.kind() == "class_body")
            {
                return Some(child);
            }
        }
        None
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        ecmascript::get_visibility(node, content)
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        ecmascript::extract_js_module_doc(src)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: TsModuleResolver = TsModuleResolver;
        Some(&RESOLVER)
    }

    fn post_process_symbols(
        &self,
        symbols: &mut Vec<crate::Symbol>,
        resolver: Option<&dyn crate::InterfaceResolver>,
        current_file: &str,
    ) {
        ecmascript::mark_interface_implementations(symbols, resolver, current_file);
    }
}

// =============================================================================
// TypeScript / TSX Module Resolver
// =============================================================================

/// Module resolver for TypeScript/TSX.
///
/// Handles:
/// - Relative imports (`./`, `../`)
/// - tsconfig.json `compilerOptions.paths` (alias mappings)
/// - tsconfig.json `compilerOptions.baseUrl` (search root)
/// - `.js` → `.ts` extension elision (TS compiles `.js` imports as `.ts`)
pub struct TsModuleResolver;

impl ModuleResolver for TsModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();
        let mut search_roots: Vec<PathBuf> = Vec::new();

        // Try to read tsconfig.json
        let tsconfig_path = root.join("tsconfig.json");
        if let Ok(content) = std::fs::read_to_string(&tsconfig_path)
            && let Ok(tsconfig) = serde_json::from_str::<serde_json::Value>(&content)
        {
            let compiler_opts = tsconfig.get("compilerOptions");

            // Parse baseUrl
            if let Some(base_url) = compiler_opts
                .and_then(|o| o.get("baseUrl"))
                .and_then(|v| v.as_str())
            {
                let base = root.join(base_url);
                search_roots.push(base);
            }

            // Parse paths aliases
            if let Some(paths) = compiler_opts
                .and_then(|o| o.get("paths"))
                .and_then(|v| v.as_object())
            {
                for (alias, targets) in paths {
                    if let Some(first) = targets
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.as_str())
                    {
                        // Strip trailing /* from alias pattern and target
                        let alias_key = alias.trim_end_matches("/*").to_string();
                        let target_path = root.join(first.trim_end_matches("/*"));
                        path_mappings.push((alias_key, target_path));
                    }
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots,
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "ts" | "tsx" | "mts" | "cts") {
            return Vec::new();
        }

        // Derive module path relative to workspace root (or first search root)
        let base = cfg.search_roots.first().unwrap_or(&cfg.workspace_root);

        let rel = file
            .strip_prefix(base)
            .or_else(|_| file.strip_prefix(&cfg.workspace_root))
            .unwrap_or(file);

        // Strip extension
        let stem = rel.with_extension("");
        let module_path = stem
            .components()
            .filter_map(|c| {
                if let std::path::Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("/");

        if module_path.is_empty() {
            return Vec::new();
        }

        vec![ModuleId {
            canonical_path: module_path,
        }]
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "ts" | "tsx" | "mts" | "cts") {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Skip node_modules / bare node_modules imports
        if raw.starts_with("node_modules/") {
            return Resolution::NotFound;
        }

        // 1. Relative imports
        if spec.is_relative || raw.starts_with("./") || raw.starts_with("../") {
            let base_dir = from_file.parent().unwrap_or(from_file);
            return resolve_ts_relative(base_dir, raw);
        }

        // 2. Path alias (tsconfig paths)
        for (alias, target_dir) in &cfg.path_mappings {
            if raw == alias || raw.starts_with(&format!("{}/", alias)) {
                let rest = raw.strip_prefix(alias).unwrap_or("");
                let rest = rest.strip_prefix('/').unwrap_or(rest);
                let candidate = if rest.is_empty() {
                    target_dir.clone()
                } else {
                    target_dir.join(rest)
                };
                let result = resolve_ts_file_candidates(&candidate);
                if !matches!(result, Resolution::NotFound) {
                    return result;
                }
            }
        }

        // 3. baseUrl-relative bare imports
        for search_root in &cfg.search_roots {
            let candidate = search_root.join(raw);
            let result = resolve_ts_file_candidates(&candidate);
            if !matches!(result, Resolution::NotFound) {
                return result;
            }
        }

        Resolution::NotFound
    }
}

/// Try .ts, .tsx, /index.ts, /index.tsx candidates for a base path.
fn resolve_ts_file_candidates(base: &Path) -> Resolution {
    // Try as-is with ts extensions
    let candidates = [
        base.with_extension("ts"),
        base.with_extension("tsx"),
        base.join("index.ts"),
        base.join("index.tsx"),
    ];
    for c in &candidates {
        if c.exists() {
            return Resolution::Resolved(c.clone(), String::new());
        }
    }
    Resolution::NotFound
}

/// Resolve a relative specifier from a directory.
fn resolve_ts_relative(base_dir: &Path, raw: &str) -> Resolution {
    // Normalize the path
    let joined = base_dir.join(raw);
    let normalized = normalize_path(&joined);

    // Strip .js extension (TS compiles .js imports as .ts)
    let base = if normalized.extension().and_then(|e| e.to_str()) == Some("js") {
        normalized.with_extension("")
    } else {
        normalized.clone()
    };

    resolve_ts_file_candidates(&base)
}

/// Simple path normalization (handle `..` components).
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the TypeScript grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "class_body",              // class body block
            "class_heritage",          // extends clause
            "class_static_block",      // static { }
            "enum_assignment",         // enum value assignment
            "enum_body",               // enum body
            "formal_parameters",       // function params
            "identifier",              // too common
            "interface_body",          // interface body
            "nested_identifier",       // a.b.c path
            "nested_type_identifier",  // a.b.Type path
            "private_property_identifier", // #field
            "property_identifier",     // obj.prop
            "public_field_definition", // class field
            "shorthand_property_identifier", // { x } shorthand
            "shorthand_property_identifier_pattern", // destructuring
            "statement_block",         // { }
            "statement_identifier",    // label name
            "switch_body",             // switch cases

            // CLAUSE
            "default_type",            // default type param
            "else_clause",             // else branch
            "extends_clause",          // class extends
            "extends_type_clause",     // T extends U
            "finally_clause",          // finally block
            "implements_clause",       // implements X

            // EXPRESSION
            "as_expression",           // x as T
            "assignment_expression",   // x = y
            "augmented_assignment_expression", // x += y
            "await_expression",        // await foo
            "call_expression",         // foo()
            "function_expression",     // function() {}
            "instantiation_expression",// generic call
            "member_expression",       // foo.bar          // new Foo()
            "non_null_expression",     // x!
            "parenthesized_expression",// (expr)
            "satisfies_expression",    // x satisfies T
            "sequence_expression",     // a, b
            "subscript_expression",    // arr[i]
            "unary_expression",        // -x, !x
            "update_expression",       // x++
            "yield_expression",        // yield x

            // TYPE NODES
            "adding_type_annotation",  // : T
            "array_type",              // T[]
            "conditional_type",        // T extends U ? V : W
            "construct_signature",     // new(): T
            "constructor_type",        // new (x: T) => U
            "existential_type",        // *
            "flow_maybe_type",         // ?T      // function sig
            "function_type",           // (x: T) => U
            "generic_type",            // T<U>
            "index_type_query",        // keyof T
            "infer_type",              // infer T
            "intersection_type",       // T & U
            "literal_type",            // "foo" type
            "lookup_type",             // T[K]
            "mapped_type_clause",      // [K in T]
            "object_type",             // { x: T }
            "omitting_type_annotation",// omit annotation
            "opting_type_annotation",  // optional annotation
            "optional_type",           // T?
            "override_modifier",       // override
            "parenthesized_type",      // (T)
            "predefined_type",         // string, number
            "readonly_type",           // readonly T
            "rest_type",               // ...T
            "template_literal_type",   // `${T}`
            "template_type",           // template type
            "this_type",               // this
            "tuple_type",              // [T, U]         // : T
            "type_arguments",          // <T, U>
            "type_assertion",          // <T>x         // type name
            "type_parameter",          // T
            "type_parameters",         // <T, U>
            "type_predicate",          // x is T
            "type_predicate_annotation", // : x is T
            "type_query",              // typeof x
            "union_type",              // T | U

            // IMPORT/EXPORT DETAILS
            "accessibility_modifier",  // public/private/protected
            "export_clause",           // export { a, b }
            "export_specifier",        // export { a as b }
            "import",                  // import keyword
            "import_alias",            // import X = Y
            "import_attribute",        // import attributes
            "import_clause",           // import clause
            "import_require_clause",   // require()
            "import_specifier",        // import { a }
            "named_imports",           // { a, b }
            "namespace_export",        // export * as ns
            "namespace_import",        // import * as ns

            // DECLARATION // abstract class // abstract method
            "ambient_declaration",     // declare
            "debugger_statement",      // debugger;
            "empty_statement",         // ;
            "expression_statement",    // expr;
            "generator_function",      // function* foo
            "generator_function_declaration", // function* declaration
            "internal_module",         // namespace/module
            "labeled_statement",       // label: stmt
            "lexical_declaration",     // let/const                  // module keyword
            "using_declaration",       // using x = ...
            "variable_declaration",    // var x
            "with_statement",          // with (obj) - deprecated
            // control flow — not extracted as symbols
            "for_in_statement",
            "switch_case",
            "continue_statement",
            "do_statement",
            "return_statement",
            "class",
            "switch_statement",
            "binary_expression",
            "while_statement",
            "for_statement",
            "if_statement",
            "throw_statement",
            "try_statement",
            "break_statement",
            "arrow_function",
            "catch_clause",
            "ternary_expression",
            "import_statement",
            "export_statement",
        ];

        validate_unused_kinds_audit(&TypeScript, documented_unused)
            .expect("TypeScript unused node kinds audit failed");
    }
}
