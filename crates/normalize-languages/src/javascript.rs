//! JavaScript language support.

use std::path::{Path, PathBuf};

use crate::ecmascript;
use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// JavaScript language support.
pub struct JavaScript;

impl Language for JavaScript {
    fn name(&self) -> &'static str {
        "JavaScript"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["js", "mjs", "cjs", "jsx"]
    }
    fn grammar_name(&self) -> &'static str {
        "javascript"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
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
        {
            let name = symbol.name.as_str();
            match symbol.kind {
                crate::SymbolKind::Function | crate::SymbolKind::Method => {
                    name.starts_with("test_")
                        || name.starts_with("Test")
                        || name == "describe"
                        || name == "it"
                        || name == "test"
                }
                crate::SymbolKind::Module => {
                    name == "tests" || name == "test" || name == "__tests__"
                }
                _ => false,
            }
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/__tests__/**/*.js",
            "**/__mocks__/**/*.js",
            "**/*.test.js",
            "**/*.spec.js",
            "**/*.test.jsx",
            "**/*.spec.jsx",
        ]
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        ecmascript::extract_decorators(node, content)
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

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        ecmascript::get_visibility(node, content)
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        ecmascript::extract_js_module_doc(src)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: JsModuleResolver = JsModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for JavaScript {}

// =============================================================================
// JavaScript Module Resolver
// =============================================================================

/// Module resolver for JavaScript (ESM and CJS).
///
/// Handles:
/// - Relative imports (`./`, `../`) — resolves `.js`, `.mjs`, `/index.js`
/// - `package.json` name field and `jsconfig.json` baseUrl
/// - Returns `NotFound` for node_modules (bare specifiers without `./`)
pub struct JsModuleResolver;

impl ModuleResolver for JsModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut search_roots: Vec<PathBuf> = Vec::new();
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        // Try jsconfig.json for path aliases
        let jsconfig_path = root.join("jsconfig.json");
        if let Ok(content) = std::fs::read_to_string(&jsconfig_path)
            && let Ok(jsconfig) = serde_json::from_str::<serde_json::Value>(&content)
        {
            let compiler_opts = jsconfig.get("compilerOptions");

            // baseUrl
            if let Some(base_url) = compiler_opts
                .and_then(|o| o.get("baseUrl"))
                .and_then(|v| v.as_str())
            {
                search_roots.push(root.join(base_url));
            }

            // paths aliases
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
        if !matches!(ext, "js" | "mjs" | "cjs" | "jsx") {
            return Vec::new();
        }

        let base = cfg.search_roots.first().unwrap_or(&cfg.workspace_root);

        let rel = file
            .strip_prefix(base)
            .or_else(|_| file.strip_prefix(&cfg.workspace_root))
            .unwrap_or(file);

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
        if !matches!(ext, "js" | "mjs" | "cjs" | "jsx") {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Skip node_modules
        if raw.starts_with("node_modules/") {
            return Resolution::NotFound;
        }

        // 1. Relative imports
        if spec.is_relative || raw.starts_with("./") || raw.starts_with("../") {
            let base_dir = from_file.parent().unwrap_or(from_file);
            let joined = base_dir.join(raw);
            let normalized = normalize_js_path(&joined);
            return resolve_js_file_candidates(&normalized);
        }

        // 2. Path alias (jsconfig paths)
        for (alias, target_dir) in &cfg.path_mappings {
            if raw == alias || raw.starts_with(&format!("{}/", alias)) {
                let rest = raw.strip_prefix(alias).unwrap_or("");
                let rest = rest.strip_prefix('/').unwrap_or(rest);
                let candidate = if rest.is_empty() {
                    target_dir.clone()
                } else {
                    target_dir.join(rest)
                };
                let result = resolve_js_file_candidates(&candidate);
                if !matches!(result, Resolution::NotFound) {
                    return result;
                }
            }
        }

        // 3. baseUrl-relative bare imports
        for search_root in &cfg.search_roots {
            let candidate = search_root.join(raw);
            let result = resolve_js_file_candidates(&candidate);
            if !matches!(result, Resolution::NotFound) {
                return result;
            }
        }

        // 4. Bare specifier without ./ — assume node_modules
        Resolution::NotFound
    }
}

/// Try .js, .mjs, /index.js candidates for a base path.
fn resolve_js_file_candidates(base: &Path) -> Resolution {
    // If it already has a js/mjs/cjs extension and exists, use it
    let base_ext = base.extension().and_then(|e| e.to_str()).unwrap_or("");
    if matches!(base_ext, "js" | "mjs" | "cjs" | "jsx") && base.exists() {
        return Resolution::Resolved(base.to_path_buf(), String::new());
    }

    let candidates = [
        base.with_extension("js"),
        base.with_extension("mjs"),
        base.with_extension("cjs"),
        base.with_extension("jsx"),
        base.join("index.js"),
        base.join("index.mjs"),
    ];
    for c in &candidates {
        if c.exists() {
            return Resolution::Resolved(c.clone(), String::new());
        }
    }
    Resolution::NotFound
}

/// Simple path normalization (handle `..` components).
fn normalize_js_path(path: &Path) -> PathBuf {
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

    /// Documents node kinds that exist in the JavaScript grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "class_body",              // class body block
            "class_heritage",          // extends clause
            "class_static_block",      // static { }
            "formal_parameters",       // function params
            "field_definition",        // class field              // too common
            "private_property_identifier", // #field
            "property_identifier",     // obj.prop
            "shorthand_property_identifier", // { x } shorthand
            "shorthand_property_identifier_pattern", // destructuring shorthand
            "statement_block",         // { }
            "statement_identifier",    // label name
            "switch_body",             // switch cases

            // CLAUSE
            "else_clause",             // else branch
            "finally_clause",          // finally block

            // EXPRESSION   // x = y
            "augmented_assignment_expression", // x += y
            "await_expression",        // await foo         // foo()     // function() {}       // foo.bar          // new Foo()
            "parenthesized_expression",// (expr)
            "sequence_expression",     // a, b
            "subscript_expression",    // arr[i]
            "unary_expression",        // -x, !x
            "update_expression",       // x++
            "yield_expression",        // yield x

            // IMPORT/EXPORT DETAILS
            "export_clause",           // export { a, b }
            "export_specifier",        // export { a as b }
            "import",                  // import keyword
            "import_attribute",        // import attributes
            "import_clause",           // import clause
            "import_specifier",        // import { a }
            "named_imports",           // { a, b }
            "namespace_export",        // export * as ns
            "namespace_import",        // import * as ns

            // DECLARATION
            "debugger_statement",      // debugger;
            "empty_statement",         // ;
            "expression_statement",    // expr;      // function* foo
            "labeled_statement",       // label: stmt     // let/const
            "using_declaration",       // using x = ...    // var x
            "with_statement",          // with (obj) - deprecated

            // JSX
            "jsx_expression",          // {expr} in JSX
            // control flow — not extracted as symbols
            "break_statement",
            "while_statement",
            "throw_statement",
            "if_statement",
            "for_statement",
            "import_statement",
            "ternary_expression",
            "catch_clause",
            "do_statement",
            "return_statement",
            "try_statement",
            "for_in_statement",
            "continue_statement",
            "switch_statement",
            "switch_case",
            "arrow_function",
        ];

        validate_unused_kinds_audit(&JavaScript, documented_unused)
            .expect("JavaScript unused node kinds audit failed");
    }
}
