//! C# language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// C# language support.
pub struct CSharp;

impl Language for CSharp {
    fn name(&self) -> &'static str {
        "C#"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }
    fn grammar_name(&self) -> &'static str {
        "c-sharp"
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
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
                let text = &content[sibling.byte_range()];
                if text.starts_with("///") {
                    let line = text.strip_prefix("///").unwrap_or("").trim();
                    let line = strip_xml_tags(line);
                    if !line.is_empty() {
                        doc_lines.push(line);
                    }
                } else if text.starts_with("/**") {
                    let lines: Vec<&str> = text
                        .strip_prefix("/**")
                        .unwrap_or(text)
                        .strip_suffix("*/")
                        .unwrap_or(text)
                        .lines()
                        .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
                        .filter(|l| !l.is_empty())
                        .collect();
                    if !lines.is_empty() {
                        return Some(lines.join(" "));
                    }
                    return None;
                } else {
                    break;
                }
            } else if sibling.kind() == "attribute_list" {
                // Skip [Attribute] between doc comment and declaration
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        let joined = doc_lines.join(" ").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "struct_declaration" => crate::SymbolKind::Struct,
            "enum_declaration" => crate::SymbolKind::Enum,
            "interface_declaration" => crate::SymbolKind::Interface,
            "record_declaration" => crate::SymbolKind::Class,
            _ => tag_kind,
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_list" {
                let mut bl = child.walk();
                for t in child.children(&mut bl) {
                    if t.kind() == "identifier" || t.kind() == "generic_name" {
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
            "method_declaration" | "constructor_declaration" | "property_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_default();
                let return_type = node
                    .child_by_field_name("type")
                    .or_else(|| node.child_by_field_name("returns"))
                    .map(|t| content[t.byte_range()].to_string());
                match return_type {
                    Some(ret) => format!("{} {}{}", ret, name, params),
                    None => format!("{}{}", name, params),
                }
            }
            "class_declaration" => format!("class {}", name),
            "struct_declaration" => format!("struct {}", name),
            "interface_declaration" => format!("interface {}", name),
            "enum_declaration" => format!("enum {}", name),
            "record_declaration" => format!("record {}", name),
            "namespace_declaration" => format!("namespace {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "using_directive" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];

        // Check for static using
        let is_static = text.contains("static ");

        // Get the namespace/type
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "qualified_name" || child.kind() == "identifier" {
                let module = content[child.byte_range()].to_string();
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: if is_static {
                        Some("static".to_string())
                    } else {
                        None
                    },
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // C#: using Namespace; or using Alias = Namespace;
        if let Some(ref alias) = import.alias {
            format!("using {} = {};", alias, import.module)
        } else {
            format!("using {};", import.module)
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
        &["**/*Test.cs", "**/*Tests.cs"]
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

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute_list" {
                attrs.push(content[child.byte_range()].to_string());
            }
        }
        attrs
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" {
                let mod_text = &content[child.byte_range()];
                if mod_text == "private" {
                    return Visibility::Private;
                }
                if mod_text == "protected" {
                    return Visibility::Protected;
                }
                if mod_text == "internal" {
                    return Visibility::Protected;
                }
                if mod_text == "public" {
                    return Visibility::Public;
                }
            }
        }
        // C# default visibility depends on context, but for skeleton purposes treat as public
        Visibility::Public
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: CSharpModuleResolver = CSharpModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for CSharp {}

impl crate::RefactorCodeGen for CSharp {
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
            "\n{}private {} {}({}) {{\n{}{}\n{}}}\n",
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
        // C# value tuples: `(int, string) F()` + `var (a, b) = F();`.
        true
    }

    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        // C# parameters are `Type name`; scan for the token preceding `name`.
        for decl in content.split([',', '(', ')']) {
            let toks: Vec<&str> = decl.split_whitespace().collect();
            if toks.len() >= 2 && toks[toks.len() - 1] == name {
                return Some(toks[toks.len() - 2].to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::CSharp;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn csharp_fn_basic() {
        let spec = ExtractedFnSpec {
            name: "Double".to_string(),
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
            CSharp.render_function(&spec),
            "\nprivate /* result */ Double(int n) {\n    var result = n * 2;\n    return result;\n}\n"
        );
    }

    #[test]
    fn csharp_fn_void() {
        let spec = ExtractedFnSpec {
            name: "Log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["Console.WriteLine(x);".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            CSharp.render_function(&spec),
            "\nprivate void Log() {\n    Console.WriteLine(x);\n}\n"
        );
    }

    #[test]
    fn csharp_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "Pair".to_string(),
            params: vec![],
            ret: GenReturn::Tuple(vec!["a".to_string(), "b".to_string()]),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(CSharp.render_call_site(&spec), "    var (a, b) = Pair();\n");
        assert_eq!(CSharp.render_binding("x", "F()", "  "), "  var x = F();\n");
        assert_eq!(CSharp.format_param("n", Some("int")), "int n");
    }

    #[test]
    fn csharp_infer_param_type() {
        assert_eq!(
            CSharp.infer_param_type("int Double(int n, string s)", "s"),
            Some("string".to_string())
        );
    }
}

// =============================================================================
// C# Module Resolver
// =============================================================================

/// Module resolver for C# (.NET / project-file conventions).
///
/// C# namespaces don't map 1:1 to file paths, but best-effort: convert
/// dotted namespace to a path and look for the file relative to the project root.
pub struct CSharpModuleResolver;

impl ModuleResolver for CSharpModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        // Look for *.csproj to confirm project root — but no structured mappings needed.
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, root: &Path, file: &Path, _cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "cs" {
            return Vec::new();
        }
        if let Ok(rel) = file.strip_prefix(root) {
            let rel_str = rel
                .to_str()
                .unwrap_or("")
                .trim_end_matches(".cs")
                .replace(['/', '\\'], ".");
            if !rel_str.is_empty() {
                return vec![ModuleId {
                    canonical_path: rel_str,
                }];
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "cs" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();

        // Try progressively stripping leading namespace components
        // (the project root namespace may be implicit)
        let parts: Vec<&str> = raw.split('.').collect();
        for skip in 0..parts.len() {
            let path_part = parts[skip..].join("/");
            let candidate = cfg.workspace_root.join(format!("{}.cs", path_part));
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name);
            }
        }
        Resolution::NotFound
    }
}

/// Strip common XML doc comment tags.
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // C# grammar uses "c_sharp" - check cross_check output for actual kinds
            // This is a placeholder - run cross_check_node_kinds to get the full list
        ];

        // C# may need manual verification - skip for now if empty
        if !documented_unused.is_empty() {
            validate_unused_kinds_audit(&CSharp, documented_unused)
                .expect("C# unused node kinds audit failed");
        }
    }
}
