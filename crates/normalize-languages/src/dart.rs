//! Dart language support.

use crate::docstring::extract_preceding_prefix_comments;
use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Dart language support.
pub struct Dart;

impl Language for Dart {
    fn name(&self) -> &'static str {
        "Dart"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }
    fn grammar_name(&self) -> &'static str {
        "dart"
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
        extract_preceding_prefix_comments(node, content, "///")
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "enum_declaration" => crate::SymbolKind::Enum,
            "mixin_declaration" => crate::SymbolKind::Trait,
            _ => tag_kind,
        }
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_dart_annotations(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "superclass" || child.kind() == "interfaces" {
                let mut ic = child.walk();
                for t in child.children(&mut ic) {
                    if t.kind() == "type_identifier" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            }
        }
        crate::ImplementsInfo {
            is_interface: false,
            implements,
        }
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
            k if k.contains("function") || k.contains("method") => {
                let return_type = node
                    .child_by_field_name("return_type")
                    .map(|t| content[t.byte_range()].to_string());
                let params = node
                    .child_by_field_name("formal_parameters")
                    .or_else(|| node.child_by_field_name("parameters"))
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                if let Some(ret) = return_type {
                    format!("{} {}{}", ret, name, params)
                } else {
                    format!("{}{}", name, params)
                }
            }
            "class_declaration" => {
                let is_abstract = node
                    .parent()
                    .map(|p| content[p.byte_range()].contains("abstract "))
                    .unwrap_or(false);
                if is_abstract {
                    format!("abstract class {}", name)
                } else {
                    format!("class {}", name)
                }
            }
            "enum_declaration" => format!("enum {}", name),
            "mixin_declaration" => format!("mixin {}", name),
            "extension_declaration" => format!("extension {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_specification" && node.kind() != "library_export" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract the import URI
        if let Some(start) = text.find('\'').or_else(|| text.find('"')) {
            // start is a byte offset; slice at it (safe: ASCII quote is single-byte) then take first char
            let Some(quote) = text[start..].chars().next() else {
                return Vec::new();
            };
            let rest = &text[start + 1..];
            if let Some(end) = rest.find(quote) {
                let module = rest[..end].to_string();
                let is_relative = module.starts_with('.') || module.starts_with('/');

                // Check for 'as' alias
                let alias = if text.contains(" as ") {
                    text.split(" as ")
                        .nth(1)
                        .and_then(|s| s.split(';').next())
                        .map(|s| s.trim().to_string())
                } else {
                    None
                };

                // Extract names from 'show' clause; 'hide' imports are named but we
                // don't suppress names so they remain empty (caller can query show node).
                // Neither show nor hide is a wildcard import.
                let (names, is_wildcard) = if text.contains(" show ") {
                    let show_names: Vec<String> = text
                        .split(" show ")
                        .nth(1)
                        .unwrap_or("")
                        .split(';')
                        .next()
                        .unwrap_or("")
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    (show_names, false)
                } else {
                    (Vec::new(), false)
                };

                return vec![Import {
                    module,
                    names,
                    alias,
                    is_wildcard,
                    is_relative,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Dart: import 'package:name/name.dart'; or import '...' show a, b, c;
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import '{}';", import.module)
        } else {
            format!(
                "import '{}' show {};",
                import.module,
                names_to_use.join(", ")
            )
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if let Some(name) = self.node_name(node, content) {
            if name.starts_with('_') {
                Visibility::Private
            } else {
                Visibility::Public
            }
        } else {
            Visibility::Public
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
        &["**/test/**/*.dart", "**/*_test.dart"]
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
        static RESOLVER: DartModuleResolver = DartModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Dart {}

impl crate::RefactorCodeGen for Dart {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        match ty {
            Some(t) => format!("{} {}", t, name),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}var {} = {};\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let ret_type = match &spec.ret {
            GenReturn::Unit => "void".to_string(),
            GenReturn::Single(v) => format!("/* {} */", v),
            GenReturn::Tuple(vs) => format!("({})", vs.join(", ")),
            GenReturn::Result(ok, _) => format!("/* {} */", ok),
        };
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} {}", ty, p.name),
                None => format!("var {}", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {};", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    return ({});", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => format!("\n{}    return {};", indent, ok),
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}{} {}({}) {{\n{}{}\n{}}}\n",
            indent, ret_type, spec.name, param_str, body, return_stmt, indent
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
            GenReturn::Single(v) => format!("{}var {} = {}({});\n", indent, v, name, args),
            GenReturn::Tuple(vs) => {
                format!("{}var ({}) = {}({});\n", indent, vs.join(", "), name, args)
            }
            GenReturn::Result(ok, _) => format!("{}var {} = {}({});\n", indent, ok, name, args),
        }
    }

    fn supports_multi_return(&self) -> bool {
        // Dart 3 records: `(int, String) f()` + `var (a, b) = f();`.
        true
    }

    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        // Dart parameters are `Type name`; scan for the token preceding `name`.
        for decl in content.split([',', '(', ')']) {
            let toks: Vec<&str> = decl.split_whitespace().collect();
            if toks.len() >= 2 && toks[toks.len() - 1] == name {
                let ty = toks[toks.len() - 2];
                // Skip Dart keyword-only modifiers that aren't a type.
                if ty != "final" && ty != "const" && ty != "var" {
                    return Some(ty.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Dart;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn dart_fn_basic() {
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
            body_lines: vec!["var result = n * 2;".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            Dart.render_function(&spec),
            "\n/* result */ double(int n) {\n    var result = n * 2;\n    return result;\n}\n"
        );
    }

    #[test]
    fn dart_fn_void() {
        let spec = ExtractedFnSpec {
            name: "log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["print(x);".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            Dart.render_function(&spec),
            "\nvoid log() {\n    print(x);\n}\n"
        );
    }

    #[test]
    fn dart_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "pair".to_string(),
            params: vec![],
            ret: GenReturn::Tuple(vec!["a".to_string(), "b".to_string()]),
            is_async: false,
            indent: "  ".to_string(),
        };
        assert_eq!(Dart.render_call_site(&spec), "  var (a, b) = pair();\n");
        assert_eq!(Dart.render_binding("x", "f()", "  "), "  var x = f();\n");
        assert_eq!(Dart.format_param("n", Some("int")), "int n");
    }

    #[test]
    fn dart_infer_param_type() {
        assert_eq!(
            Dart.infer_param_type("int double(int n, String s)", "s"),
            Some("String".to_string())
        );
    }
}

// =============================================================================
// Dart Module Resolver
// =============================================================================

/// Module resolver for Dart (pub package conventions).
///
/// Reads `pubspec.yaml` to find the package name. Resolves:
/// - `package:mypackage/src/foo.dart` → `lib/src/foo.dart`
/// - `dart:core` etc. → `NotFound` (SDK)
/// - relative imports → resolved relative to `from_file`
pub struct DartModuleResolver;

impl ModuleResolver for DartModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let pubspec = root.join("pubspec.yaml");
        if let Ok(content) = std::fs::read_to_string(&pubspec) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("name:") {
                    let name = rest.trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        path_mappings.push((name.to_string(), root.join("lib")));
                        break;
                    }
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: vec![root.join("lib")],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "dart" {
            return Vec::new();
        }
        for (pkg_name, lib_dir) in &cfg.path_mappings {
            if let Ok(rel) = file.strip_prefix(lib_dir) {
                let rel_str = rel.to_str().unwrap_or("").replace('\\', "/");
                let canonical = format!("package:{}/{}", pkg_name, rel_str);
                return vec![ModuleId {
                    canonical_path: canonical,
                }];
            }
        }
        // file under workspace root but not lib/
        if let Ok(rel) = file.strip_prefix(&cfg.workspace_root) {
            return vec![ModuleId {
                canonical_path: rel.to_str().unwrap_or("").replace('\\', "/"),
            }];
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "dart" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;

        // dart: SDK imports
        if raw.starts_with("dart:") {
            return Resolution::NotFound;
        }

        // Relative imports
        if raw.starts_with('.') {
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
            return Resolution::NotFound;
        }

        // package: imports
        if let Some(rest) = raw.strip_prefix("package:") {
            // package:pkgname/path/to/file.dart
            let slash = rest.find('/');
            let (pkg, path_in_pkg) = if let Some(idx) = slash {
                (&rest[..idx], &rest[idx + 1..])
            } else {
                (rest, "")
            };

            // Check if it's our own package
            for (own_pkg, lib_dir) in &cfg.path_mappings {
                if pkg == own_pkg {
                    let candidate = lib_dir.join(path_in_pkg);
                    if candidate.exists() {
                        let name = candidate
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        return Resolution::Resolved(candidate, name);
                    }
                    return Resolution::NotFound;
                }
            }
        }

        Resolution::NotFound
    }
}

/// Extract Dart annotations from child and preceding sibling nodes.
fn extract_dart_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "annotation" {
            let text = content[child.byte_range()].trim().to_string();
            if !text.is_empty() {
                attrs.push(text);
            }
        }
    }
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "annotation" {
            let text = content[sibling.byte_range()].trim().to_string();
            if !text.is_empty() {
                attrs.insert(0, text);
            }
            prev = sibling.prev_sibling();
        } else {
            break;
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "additive_expression", "additive_operator", "annotation", "as_operator",
            "assert_statement", "assignable_expression", "assignment_expression",
            "assignment_expression_without_cascade", "await_expression", "binary_operator",
            "bitwise_and_expression", "bitwise_operator", "bitwise_or_expression",
            "bitwise_xor_expression", "cascade_section", "case_builtin",
            "catch_parameters", "class_body", "const_object_expression",
            "constant_constructor_signature", "constructor_invocation",
            "constructor_param", "constructor_signature", "constructor_tearoff",
            "declaration", "dotted_identifier_list", "enum_body", "enum_constant",
            "equality_expression", "equality_operator", "expression_statement",
            "extension_body", "extension_type_declaration", "factory_constructor_signature",
            "finally_clause", "for_element", "for_loop_parts", "formal_parameter",
            "formal_parameter_list", "function_expression_body", "function_type",
            "identifier_dollar_escaped", "identifier_list",
            "if_element", "if_null_expression", "import_or_export", "increment_operator",
            "inferred_type", "initialized_identifier", "initialized_identifier_list",
            "initialized_variable_definition", "initializer_list_entry", "interface",
            "interfaces", "is_operator", "label", "lambda_expression",
            "library_import", "library_name", "local_function_declaration",
            "local_variable_declaration", "logical_and_operator", "logical_or_operator",
            "minus_operator", "mixin_application_class", "multiplicative_expression",
            "multiplicative_operator", "named_parameter_types", "negation_operator",
            "new_expression", "normal_parameter_type", "nullable_type",
            "operator_signature", "optional_formal_parameters", "optional_parameter_types",
            "optional_positional_parameter_types", "parameter_type_list",
            "parenthesized_expression", "pattern_variable_declaration",
            "postfix_expression", "postfix_operator", "prefix_operator", "qualified",
            "record_type", "record_type_field", "record_type_named_field",
            "redirecting_factory_constructor_signature", "relational_expression",
            "relational_operator", "representation_declaration", "rethrow_builtin",
            "scoped_identifier", "shift_expression", "shift_operator", "spread_element",
            "static_final_declaration", "static_final_declaration_list", "superclass",
            "super_formal_parameter", "switch_block", "switch_expression",
            "switch_expression_case", "switch_statement_default", "symbol_literal",
            "throw_expression_without_cascade", "tilde_operator", "type_arguments",
            "type_bound", "type_cast", "type_cast_expression", "type_identifier",
            "type_parameter", "type_parameters", "type_test", "type_test_expression",
            "typed_identifier", "unary_expression", "void_type", "yield_each_statement",
            "yield_statement",
            // control flow — not extracted as symbols
            "logical_and_expression",
            "for_statement",
            "do_statement",
            "try_statement",
            "return_statement",
            "continue_statement",
            "catch_clause",
            "conditional_expression",
            "break_statement",
            "switch_statement_case",
            "block",
            "switch_statement",
            "if_statement",
            "throw_expression",
            "rethrow_expression",
            "function_body",
            "function_expression",
            "logical_or_expression",
            "library_export",
            "while_statement",
            "import_specification",
            "method_signature",
            "type_alias",
        ];
        validate_unused_kinds_audit(&Dart, documented_unused)
            .expect("Dart unused node kinds audit failed");
    }
}
