//! Groovy language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// Groovy language support.
pub struct Groovy;

impl Language for Groovy {
    fn name(&self) -> &'static str {
        "Groovy"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["groovy", "gradle", "gvy", "gy", "gsh"]
    }
    fn grammar_name(&self) -> &'static str {
        "groovy"
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // function_definition uses "function" field instead of "name"
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("function"))
            .map(|n| &content[n.byte_range()])
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_groovydoc(node, content)
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_groovy_annotations(node, content)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "groovy_import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import foo.bar.Baz or import foo.bar.*
        if let Some(rest) = text.strip_prefix("import ") {
            let rest = rest.strip_prefix("static ").unwrap_or(rest);
            let module = rest.trim().trim_end_matches(';').to_string();
            let is_wildcard = module.ends_with(".*");

            return vec![Import {
                module: module.trim_end_matches(".*").to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard,
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Groovy: import pkg.Class or import pkg.*
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if import.is_wildcard {
            format!("import {}.*", import.module)
        } else if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else if names_to_use.len() == 1 {
            format!("import {}.{}", import.module, names_to_use[0])
        } else {
            // Groovy doesn't have multi-import syntax, so format as module
            format!("import {}", import.module)
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.starts_with("private") {
            Visibility::Private
        } else if text.starts_with("protected") {
            Visibility::Protected
        } else {
            Visibility::Public
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let has_test_attr = symbol.attributes.iter().any(|a| a.contains("@Test"));
        if has_test_attr {
            return true;
        }
        match symbol.kind {
            crate::SymbolKind::Class => {
                symbol.name.starts_with("Test") || symbol.name.ends_with("Test")
            }
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/src/test/**/*.groovy",
            "**/*Test.groovy",
            "**/*Spec.groovy",
        ]
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
        static RESOLVER: GroovyModuleResolver = GroovyModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Groovy {}

impl crate::RefactorCodeGen for Groovy {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        // Groovy is optionally typed; default to `def name`.
        match ty {
            Some(t) => format!("{} {}", t, name),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}def {} = {}\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} {}", ty, p.name),
                None => format!("def {}", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {}", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    return [{}]", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => format!("\n{}    return {}", indent, ok),
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}def {}({}) {{\n{}{}\n{}}}\n",
            indent, spec.name, param_str, body, return_stmt, indent
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
            GenReturn::Single(v) => format!("{}def {} = {}({})\n", indent, v, name, args),
            GenReturn::Tuple(vs) => {
                format!("{}def ({}) = {}({})\n", indent, vs.join(", "), name, args)
            }
            GenReturn::Result(ok, _) => format!("{}def {} = {}({})\n", indent, ok, name, args),
        }
    }

    fn supports_multi_return(&self) -> bool {
        // Groovy list-destructuring: `return [a, b]` + `def (a, b) = f()`.
        true
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Groovy;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn groovy_fn_basic() {
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
            body_lines: vec!["def result = n * 2".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            Groovy.render_function(&spec),
            "\ndef double(int n) {\n    def result = n * 2\n    return result\n}\n"
        );
    }

    #[test]
    fn groovy_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "pair".to_string(),
            params: vec![],
            ret: GenReturn::Tuple(vec!["a".to_string(), "b".to_string()]),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(Groovy.render_call_site(&spec), "    def (a, b) = pair()\n");
        assert_eq!(Groovy.render_binding("x", "f()", "  "), "  def x = f()\n");
        assert_eq!(Groovy.format_param("n", None), "n");
        assert_eq!(Groovy.format_param("n", Some("int")), "int n");
    }
}

// =============================================================================
// Groovy Module Resolver
// =============================================================================

/// Module resolver for Groovy (Maven/Gradle conventions).
pub struct GroovyModuleResolver;

const GROOVY_SRC_DIRS: &[&str] = &["src/main/groovy", "src/test/groovy", ""];

impl ModuleResolver for GroovyModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: GROOVY_SRC_DIRS.iter().map(|d| root.join(d)).collect(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "groovy" && ext != "gradle" && ext != "gvy" && ext != "gy" && ext != "gsh" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let rel_str = rel.to_str().unwrap_or("");
                // Strip known extensions
                let stripped = rel_str
                    .trim_end_matches(".groovy")
                    .trim_end_matches(".gradle")
                    .trim_end_matches(".gvy")
                    .trim_end_matches(".gy")
                    .trim_end_matches(".gsh");
                let canonical = stripped.replace(['/', '\\'], ".");
                if !canonical.is_empty() {
                    return vec![ModuleId {
                        canonical_path: canonical,
                    }];
                }
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "groovy" && ext != "gradle" && ext != "gvy" && ext != "gy" && ext != "gsh" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        let path_part = raw.replace('.', "/");
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();
        for search_root in &cfg.search_roots {
            let candidate = search_root.join(format!("{}.groovy", path_part));
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name);
            }
        }
        Resolution::NotFound
    }
}

/// Extract a GroovyDoc comment from a node.
///
/// The Groovy tree-sitter grammar wraps documented declarations in a `groovy_doc`
/// parent node rather than making the doc comment a sibling. This function checks
/// the parent node for `groovy_doc` and extracts the doc text from `first_line`
/// and `tag_value` children.
fn extract_groovydoc(node: &Node, content: &str) -> Option<String> {
    let parent = node.parent()?;
    if parent.kind() != "groovy_doc" {
        return None;
    }

    let mut doc_parts: Vec<String> = Vec::new();
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        match child.kind() {
            "first_line" => {
                let text = content[child.byte_range()].trim();
                // first_line may include trailing */
                let text = text.strip_suffix("*/").unwrap_or(text).trim();
                if !text.is_empty() {
                    doc_parts.push(text.to_string());
                }
            }
            "tag_value" => {
                let text = content[child.byte_range()].trim();
                if !text.is_empty() {
                    doc_parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    if doc_parts.is_empty() {
        None
    } else {
        Some(doc_parts.join(" "))
    }
}

/// Extract annotations from a Groovy definition node.
///
/// Groovy annotations (`@Grab`, `@Test`, etc.) appear as `annotation`
/// children of the definition node (when no GroovyDoc is present).
fn extract_groovy_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "annotation" {
            attrs.push(content[child.byte_range()].to_string());
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
            "access_modifier", "array_type", "builtintype", "declaration",
            "do_while_loop", "dotted_identifier", "for_parameters",
            "function_call", "function_declaration", "groovy_doc_throws",
            "identifier", "juxt_function_call", "modifier",
            "parenthesized_expression", "qualified_name", "return", "switch_block",
            "type_with_generics", "wildcard_import",
            // control flow — not extracted as symbols
            "switch_statement",
            "if_statement",
            "groovy_import",
            "while_loop",
            "try_statement",
            "for_in_loop",
            "for_loop",
            "case",
        ];
        validate_unused_kinds_audit(&Groovy, documented_unused)
            .expect("Groovy unused node kinds audit failed");
    }
}
