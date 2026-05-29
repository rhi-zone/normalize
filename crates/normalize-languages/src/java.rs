//! Java language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// Java language support.
pub struct Java;

impl Language for Java {
    fn name(&self) -> &'static str {
        "Java"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
    fn grammar_name(&self) -> &'static str {
        "java"
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
        extract_javadoc(node, content)
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_annotations(node, content)
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "enum_declaration" => crate::SymbolKind::Enum,
            "interface_declaration" | "annotation_type_declaration" => crate::SymbolKind::Interface,
            "record_declaration" => crate::SymbolKind::Struct,
            _ => tag_kind,
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "superclass" {
                let mut sc = child.walk();
                for t in child.children(&mut sc) {
                    if t.kind() == "type_identifier" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            } else if child.kind() == "super_interfaces" {
                let mut si = child.walk();
                for list in child.children(&mut si) {
                    if list.kind() == "type_list" {
                        let mut tc = list.walk();
                        for t in list.children(&mut tc) {
                            if t.kind() == "type_identifier" {
                                implements.push(content[t.byte_range()].to_string());
                            }
                        }
                    }
                }
            }
        }
        crate::ImplementsInfo {
            is_interface: node.kind() == "interface_declaration",
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
            "method_declaration" | "constructor_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                format!("{}{}", name, params)
            }
            "class_declaration" => format!("class {}", name),
            "interface_declaration" => format!("interface {}", name),
            "enum_declaration" => format!("enum {}", name),
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

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];

        // Extract import path
        let is_static = text.contains("static ");
        let is_wildcard = text.contains(".*");

        // Get the scoped_identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                let module = content[child.byte_range()].to_string();
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: if is_static {
                        Some("static".to_string())
                    } else {
                        None
                    },
                    is_wildcard,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Java: import pkg.Class; or import pkg.*;
        if import.is_wildcard {
            format!("import {}.*;", import.module)
        } else {
            format!("import {};", import.module)
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
            "**/src/test/**/*.java",
            "**/Test*.java",
            "**/*Test.java",
            "**/*Tests.java",
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

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mods = &content[child.byte_range()];
                if mods.contains("private") {
                    return Visibility::Private;
                }
                if mods.contains("protected") {
                    return Visibility::Protected;
                }
                // public or no modifier = visible in skeleton
                return Visibility::Public;
            }
        }
        // No modifier = package-private, but still visible for skeleton purposes
        Visibility::Public
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: JavaModuleResolver = JavaModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Java {}

impl crate::RefactorCodeGen for Java {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        // Java is not a recipe target for add-parameter today; match the recipe's
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
        let ret_type = match &spec.ret {
            GenReturn::Unit => "void".to_string(),
            GenReturn::Single(v) => format!("/* {} */", v),
            GenReturn::Tuple(vs) => format!("/* TODO: struct({}) */", vs.join(", ")),
            GenReturn::Result(ok, err) => format!("/* {} throws {} */", ok, err),
        };
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} {}", ty, p.name),
                None => format!("/* type */ {}", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {};", indent, v),
            GenReturn::Tuple(vs) => {
                format!("\n{}    // TODO: return struct({});", indent, vs.join(", "))
            }
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
            GenReturn::Tuple(vs) => format!(
                "{}var result = {}({}); // TODO: unpack ({})\n",
                indent,
                name,
                args,
                vs.join(", ")
            ),
            GenReturn::Result(ok, _) => format!("{}var {} = {}({});\n", indent, ok, name, args),
        }
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Java;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn java_fn_basic() {
        let spec = ExtractedFnSpec {
            name: "doubleIt".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: Some("int".to_string()),
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            is_generator: false,
            body_lines: vec!["int result = n * 2;".to_string()],
            indent: String::new(),
        };
        let out = Java.render_function(&spec);
        assert_eq!(
            out,
            "\nprivate /* result */ doubleIt(int n) {\n    int result = n * 2;\n    return result;\n}\n"
        );
    }

    #[test]
    fn java_fn_void() {
        let spec = ExtractedFnSpec {
            name: "log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["System.out.println(x);".to_string()],
            indent: String::new(),
        };
        let out = Java.render_function(&spec);
        assert_eq!(
            out,
            "\nprivate void log() {\n    System.out.println(x);\n}\n"
        );
    }

    #[test]
    fn java_call_site_single() {
        let spec = CallSiteSpec {
            name: "doubleIt".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: Some("int".to_string()),
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(
            Java.render_call_site(&spec),
            "    var result = doubleIt(n);\n"
        );
    }

    #[test]
    fn java_binding_and_param() {
        assert_eq!(
            Java.render_binding("x", "compute()", "  "),
            "  let x = compute();\n"
        );
        assert_eq!(Java.format_param("n", Some("int")), "n: int");
        assert_eq!(Java.format_param("n", None), "n");
    }
}

// =============================================================================
// Java Module Resolver
// =============================================================================

/// Module resolver for Java (Maven/Gradle conventions).
///
/// Java package = directory hierarchy. `com.example.Foo` lives at
/// `src/main/java/com/example/Foo.java` (or `src/test/java/...`).
pub struct JavaModuleResolver;

/// Source directory prefixes to search under workspace root.
const JAVA_SRC_DIRS: &[&str] = &["src/main/java", "src/test/java", ""];

impl ModuleResolver for JavaModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: JAVA_SRC_DIRS.iter().map(|d| root.join(d)).collect(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "java" {
            return Vec::new();
        }
        for search_root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(search_root) {
                let rel_str = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".java")
                    .replace(['/', '\\'], ".");
                if !rel_str.is_empty() {
                    return vec![ModuleId {
                        canonical_path: rel_str,
                    }];
                }
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "java" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;
        // Convert dotted package to path: com.example.Foo → com/example/Foo.java
        let path_part = raw.replace('.', "/");
        let file_name = format!("{}.java", path_part);
        let exported_name = raw.rsplit('.').next().unwrap_or(raw).to_string();

        for search_root in &cfg.search_roots {
            let candidate = search_root.join(&file_name);
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name);
            }
        }

        Resolution::NotFound
    }
}

/// Extract a JavaDoc comment (`/** ... */`) preceding a node.
///
/// Walks backwards through siblings looking for a `block_comment` starting with `/**`.
fn extract_javadoc(node: &Node, content: &str) -> Option<String> {
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        match sibling.kind() {
            "block_comment" => {
                let text = &content[sibling.byte_range()];
                if text.starts_with("/**") {
                    return Some(clean_block_doc_comment(text));
                }
                return None;
            }
            "line_comment" => {
                // Skip line comments, keep looking for a block comment
            }
            "modifiers" | "marker_annotation" | "annotation" => {
                // Skip annotations/modifiers between doc comment and declaration
            }
            _ => return None,
        }
        prev = sibling.prev_sibling();
    }
    None
}

/// Clean a `/** ... */` block doc comment into plain text.
fn clean_block_doc_comment(text: &str) -> String {
    let lines: Vec<&str> = text
        .strip_prefix("/**")
        .unwrap_or(text)
        .strip_suffix("*/")
        .unwrap_or(text)
        .lines()
        .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join(" ")
}

/// Extract annotations from a Java definition node.
fn extract_annotations(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(modifiers) = node.child_by_field_name("modifiers").or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor).find(|c| c.kind() == "modifiers")
    }) {
        let mut cursor = modifiers.walk();
        for child in modifiers.children(&mut cursor) {
            if child.kind() == "marker_annotation" || child.kind() == "annotation" {
                attrs.push(content[child.byte_range()].to_string());
            }
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the Java grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "block_comment",           // comments
            "class_body",              // class body
            "class_literal",           // Foo.class
            "constructor_body",        // constructor body
            "enum_body",               // enum body
            "enum_body_declarations",  // enum body decls
            "enum_constant",           // enum value
            "field_declaration",       // field decl
            "formal_parameter",        // method param
            "formal_parameters",       // param list
            "identifier",              // too common
            "interface_body",          // interface body
            "modifiers",               // access modifiers
            "scoped_identifier",       // pkg.Class
            "scoped_type_identifier",  // pkg.Type              // extends
            "super_interfaces",        // implements         // type name

            // CLAUSE
            "catch_formal_parameter",  // catch param
            "catch_type",              // catch type
            "extends_interfaces",      // extends for interfaces
            "finally_clause",          // finally block
            "switch_block",            // switch body
            "switch_block_statement_group", // case group
            "throws",                  // throws clause

            // EXPRESSION
            "array_creation_expression", // new T[]
            "assignment_expression",   // x = y
            "cast_expression",         // (T)x
            "instanceof_expression",   // x instanceof T
            "lambda_expression",       // x -> y       // obj.method()
            "method_reference",        // Class::method // new Foo()
            "parenthesized_expression",// (expr)
            "template_expression",     // string template
            "unary_expression",        // -x, !x
            "update_expression",       // x++
            "yield_statement",         // yield x

            // TYPE
            "annotated_type",          // @Ann Type
            "array_type",              // T[]
            "boolean_type",            // boolean
            "floating_point_type",     // float, double
            "generic_type",            // T<U>
            "integral_type",           // int, long
            "type_arguments",          // <T, U>
            "type_bound",              // T extends X               // T, U, V
            "type_parameter",          // T
            "type_parameters",         // <T, U>
            "type_pattern",            // type pattern
            "void_type",               // void

            // DECLARATION
            "annotation_type_body",    // @interface body
            "annotation_type_declaration", // @interface
            "annotation_type_element_declaration", // @interface element
            "assert_statement",        // assert
            "compact_constructor_declaration", // record constructor
            "constant_declaration",    // const decl
            "explicit_constructor_invocation", // this(), super()
            "expression_statement",    // expr;
            "labeled_statement",       // label: stmt
            "local_variable_declaration", // local var
            "record_declaration",      // record
            "record_pattern_body",     // record pattern

            // MODULE
            "exports_module_directive",// exports
            "module_body",             // module body
            "module_declaration",      // module
            "opens_module_directive",  // opens
            "package_declaration",     // package
            "provides_module_directive", // provides
            "requires_modifier",       // requires modifier
            "requires_module_directive", // requires
            "uses_module_directive",   // uses

            // OTHER
            "resource_specification", // try-with-resources
            "synchronized_statement",  // synchronized
            "try_with_resources_statement", // try-with
            // control flow — not extracted as symbols
            "do_statement",
            "return_statement",
            "constructor_declaration",
            "binary_expression",
            "try_statement",
            "continue_statement",
            "switch_expression",
            "ternary_expression",
            "while_statement",
            "break_statement",
            "enhanced_for_statement",
            "import_declaration",
            "for_statement",
            "block",
            "throw_statement",
            "catch_clause",
            "if_statement",
        ];

        validate_unused_kinds_audit(&Java, documented_unused)
            .expect("Java unused node kinds audit failed");
    }
}
