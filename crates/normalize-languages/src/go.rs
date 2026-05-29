//! Go language support.

use std::path::{Path, PathBuf};

use crate::docstring::extract_preceding_prefix_comments;
use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// Go language support.
pub struct Go;

impl Language for Go {
    fn name(&self) -> &'static str {
        "Go"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }
    fn grammar_name(&self) -> &'static str {
        "go"
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
        extract_preceding_prefix_comments(node, content, "//")
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        // Go type_spec wraps the actual type (struct_type, interface_type, etc.)
        if node.kind() == "type_spec"
            && let Some(type_node) = node.child_by_field_name("type")
        {
            return match type_node.kind() {
                "struct_type" => crate::SymbolKind::Struct,
                "interface_type" => crate::SymbolKind::Interface,
                _ => tag_kind,
            };
        }
        tag_kind
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
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                format!("func {}{}", name, params)
            }
            "type_spec" => format!("type {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let mut imports = Vec::new();
        let line = node.start_position().row + 1;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "import_spec" => {
                    // import "path" or import alias "path"
                    if let Some(imp) = Self::parse_import_spec(&child, content, line) {
                        imports.push(imp);
                    }
                }
                "import_spec_list" => {
                    // Grouped imports
                    let mut list_cursor = child.walk();
                    for spec in child.children(&mut list_cursor) {
                        if spec.kind() == "import_spec"
                            && let Some(imp) = Self::parse_import_spec(&spec, content, line)
                        {
                            imports.push(imp);
                        }
                    }
                }
                _ => {}
            }
        }

        imports
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Go: import "pkg" or import alias "pkg"
        if let Some(ref alias) = import.alias {
            format!("import {} \"{}\"", alias, import.module)
        } else {
            format!("import \"{}\"", import.module)
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let is_exported = self
            .node_name(node, content)
            .and_then(|n| n.chars().next())
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        if is_exported {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        match symbol.kind {
            crate::SymbolKind::Function => {
                let name = symbol.name.as_str();
                name.starts_with("Test")
                    || name.starts_with("Benchmark")
                    || name.starts_with("Example")
            }
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/*_test.go"]
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        extract_go_package_doc(src)
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
        static RESOLVER: GoModuleResolver = GoModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Go {}

impl crate::RefactorCodeGen for Go {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        // Go is not a recipe target for add-parameter today; match the recipe's
        // generic default (`name: type`) so migration is behaviour-preserving.
        match ty {
            Some(t) => format!("{}: {}", name, t),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        // Matches the recipe's generic default binding form.
        format!("{}let {} = {};\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} {}", p.name, ty),
                None => format!("{} interface{{}}", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let ret_str = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!(" /* {} */", v),
            GenReturn::Tuple(vs) => format!(" ({} /* multi-return */)", vs.join(", ")),
            GenReturn::Result(ok, _) => format!(" ({}, error)", ok),
        };
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {}", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    return {}", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => format!("\n{}    return {}, nil", indent, ok),
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}func {}({}){} {{\n{}{}\n{}}}\n",
            indent, spec.name, param_str, ret_str, body, return_stmt, indent
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
            GenReturn::Unit => format!("{}{}({})\n", indent, name, args),
            GenReturn::Single(v) => format!("{}{} := {}({})\n", indent, v, name, args),
            GenReturn::Tuple(vs) => {
                format!("{}{} := {}({})\n", indent, vs.join(", "), name, args)
            }
            GenReturn::Result(ok, _) => format!(
                "{}{}, err := {}({})\n{}if err != nil {{ return err }}\n",
                indent, ok, name, args, indent
            ),
        }
    }

    fn supports_multi_return(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Go;
    use crate::{ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn go_fn_basic() {
        let spec = ExtractedFnSpec {
            name: "double".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: Some("int".to_string()),
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            is_generator: false,
            body_lines: vec!["result := n * 2".to_string()],
            indent: String::new(),
        };
        let out = Go.render_function(&spec);
        assert!(out.contains("func double(n int)"));
        assert!(out.contains("return result"));
    }
}

// =============================================================================
// Go Module Resolver
// =============================================================================

/// Module resolver for Go.
///
/// Uses `go.mod` at the workspace root to extract the module path.
/// In Go, a package = a directory, so all `.go` files in the same directory
/// belong to the same package (same import path).
pub struct GoModuleResolver;

impl ModuleResolver for GoModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let go_mod = root.join("go.mod");
        if let Ok(content) = std::fs::read_to_string(&go_mod) {
            // Parse `module <path>` line
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(module_path) = trimmed.strip_prefix("module ") {
                    let module_path = module_path.trim().to_string();
                    path_mappings.push((module_path, root.to_path_buf()));
                    break;
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: Vec::new(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "go" {
            return Vec::new();
        }

        // Package import path = module path + path from root to file's directory
        for (module_path, module_root) in &cfg.path_mappings {
            let file_dir = match file.parent() {
                Some(d) => d,
                None => continue,
            };
            if let Ok(rel) = file_dir.strip_prefix(module_root) {
                let rel_str = rel
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

                let canonical = if rel_str.is_empty() {
                    module_path.clone()
                } else {
                    format!("{}/{}", module_path, rel_str)
                };
                return vec![ModuleId {
                    canonical_path: canonical,
                }];
            }
        }

        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "go" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Check if the import path starts with our module path
        for (module_path, module_root) in &cfg.path_mappings {
            if raw == module_path {
                // Importing the root package itself
                return Resolution::Resolved(module_root.clone(), String::new());
            }
            if let Some(rest) = raw.strip_prefix(&format!("{}/", module_path)) {
                // rest is the subdirectory path within the module
                let target_dir = module_root.join(rest);
                if target_dir.is_dir() {
                    return Resolution::Resolved(target_dir, String::new());
                }
                return Resolution::NotFound;
            }
        }

        // Not in this module (stdlib or third-party)
        Resolution::NotFound
    }
}

/// Extract the Go package comment from source.
///
/// The Go convention is a block of `//` comments immediately before
/// the `package` keyword. Scans backwards from the `package` line.
/// A blank line between the comment and `package` means it is NOT a doc comment.
fn extract_go_package_doc(src: &str) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    // Find the package declaration line
    let pkg_idx = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with("package ") || t == "package"
    })?;

    // A blank line immediately before package means no doc comment
    if pkg_idx > 0 && lines[pkg_idx - 1].trim().is_empty() {
        return None;
    }

    // Collect comment lines immediately preceding the package line
    let mut doc_lines: Vec<&str> = Vec::new();
    let mut idx = pkg_idx;
    while idx > 0 {
        idx -= 1;
        let t = lines[idx].trim();
        if t.starts_with("//") {
            doc_lines.push(t);
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    // Reverse to get lines in original order and strip `//` prefix
    doc_lines.reverse();
    let text = doc_lines
        .iter()
        .map(|l| l.trim_start_matches("//").trim_start())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() { None } else { Some(text) }
}

impl Go {
    fn parse_import_spec(node: &Node, content: &str, line: usize) -> Option<Import> {
        let mut path = String::new();
        let mut alias = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "interpreted_string_literal" => {
                    let text = &content[child.byte_range()];
                    path = text.trim_matches('"').to_string();
                }
                "package_identifier" | "blank_identifier" | "dot" => {
                    alias = Some(content[child.byte_range()].to_string());
                }
                _ => {}
            }
        }

        if path.is_empty() {
            return None;
        }

        let is_wildcard = alias.as_deref() == Some(".");
        Some(Import {
            module: path,
            names: Vec::new(),
            alias,
            is_wildcard,
            is_relative: false, // Go doesn't have relative imports in the traditional sense
            line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents node kinds that exist in the Go grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        use crate::validate_unused_kinds_audit;

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "blank_identifier",        // _
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_identifier",        // field name              // too common          // package foo
            "package_identifier",      // package name
            "parameter_declaration",   // func param
            "statement_list",          // block contents
            "variadic_parameter_declaration", // ...T

            // CLAUSE
            "default_case",            // default:
            "for_clause",              // for init; cond; post
            "import_spec",             // import spec
            "import_spec_list",        // import block
            "method_elem",             // interface method
            "range_clause",            // for range

            // EXPRESSION         // foo()
            "index_expression",        // arr[i]// (expr)     // foo.bar
            "slice_expression",        // arr[1:3]
            "type_assertion_expression", // x.(T)
            "type_conversion_expression", // T(x)
            "type_instantiation_expression", // generic instantiation
            "unary_expression",        // -x, !x

            // TYPE
            "array_type",              // [N]T
            "channel_type",            // chan T
            "implicit_length_array_type", // [...]T
            "function_type",           // func(T) U
            "generic_type",            // T[U]
            "interface_type",          // interface{}
            "map_type",                // map[K]V
            "negated_type",            // ~T
            "parenthesized_type",      // (T)
            "pointer_type",            // *T
            "qualified_type",          // pkg.Type
            "slice_type",              // []T
            "struct_type",             // struct{}
            "type_arguments",          // [T, U]
            "type_constraint",         // T constraint
            "type_elem",               // type element         // type name
            "type_parameter_declaration", // [T any]
            "type_parameter_list",     // type params

            // DECLARATION
            "assignment_statement",    // x = y       // const x = 1
            "dec_statement",           // x--
            "expression_list",         // a, b, c
            "expression_statement",    // expr
            "inc_statement",           // x++
            "short_var_declaration",   // x := y
            "type_alias",              // type X = Y        // type X struct{}         // var x int

            // CONTROL FLOW DETAILS
            "empty_statement",         // ;
            "fallthrough_statement",   // fallthrough
            "go_statement",            // go foo()
            "labeled_statement",       // label:
            "receive_statement",       // <-ch
            "send_statement",          // ch <- x
            // control flow — not extracted as symbols
            "return_statement",
            "continue_statement",
            "break_statement",
            "if_statement",
            "for_statement",
            "goto_statement",
            "expression_switch_statement",
            "expression_case",
            "type_case",
            "type_switch_statement",
            "select_statement",
            "block",
            "defer_statement",
            "binary_expression",
            "communication_case",
        ];

        validate_unused_kinds_audit(&Go, documented_unused)
            .expect("Go unused node kinds audit failed");
    }
}
