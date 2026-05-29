//! D language support.

use std::path::{Path, PathBuf};

use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// D language support.
pub struct D;

impl D {
    /// Recursively collect type names from a D inheritance clause.
    /// D nests: base_class_list > super_class_or_interface/interfaces/interface >
    /// qualified_identifier > identifier(s)
    fn collect_identifiers(node: &Node, content: &str, out: &mut Vec<String>) {
        if node.kind() == "qualified_identifier" {
            out.push(content[node.byte_range()].to_string());
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_identifiers(&child, content, out);
        }
    }
}

impl Language for D {
    fn name(&self) -> &'static str {
        "D"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["d", "di"]
    }
    fn grammar_name(&self) -> &'static str {
        "d"
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
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();
        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            match sibling.kind() {
                "comment" => {
                    if text.starts_with("///") {
                        let line = text.strip_prefix("///").unwrap_or(text).trim();
                        if !line.is_empty() {
                            doc_lines.push(line.to_string());
                        }
                        prev = sibling.prev_sibling();
                    } else {
                        break;
                    }
                }
                "block_comment" => {
                    if text.starts_with("/**") {
                        let inner = text
                            .strip_prefix("/**")
                            .unwrap_or(text)
                            .strip_suffix("*/")
                            .unwrap_or(text);
                        for line in inner.lines() {
                            let clean = line.trim().strip_prefix('*').unwrap_or(line).trim();
                            if !clean.is_empty() {
                                doc_lines.push(clean.to_string());
                            }
                        }
                    }
                    break;
                }
                "nesting_block_comment" => {
                    if text.starts_with("/++") {
                        let inner = text
                            .strip_prefix("/++")
                            .unwrap_or(text)
                            .strip_suffix("+/")
                            .unwrap_or(text);
                        for line in inner.lines() {
                            let clean = line.trim().strip_prefix('+').unwrap_or(line).trim();
                            if !clean.is_empty() {
                                doc_lines.push(clean.to_string());
                            }
                        }
                    }
                    break;
                }
                _ => break,
            }
        }
        if doc_lines.is_empty() {
            return None;
        }
        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute_specifier" {
                let text = content[child.byte_range()].trim().to_string();
                if !text.is_empty() {
                    attrs.push(text);
                }
            }
        }
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_specifier" {
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

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = self.node_name(node, content).unwrap_or("");
        match node.kind() {
            "module_declaration" => format!("module {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_list" {
                D::collect_identifiers(&child, content, &mut implements);
            }
        }
        crate::ImplementsInfo {
            is_interface: false,
            implements,
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import_declaration" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        // Strip "import " prefix and trailing ";"
        let module = text
            .trim()
            .strip_prefix("import ")
            .unwrap_or(text.trim())
            .trim_end_matches(';')
            .trim()
            .to_string();
        let is_wildcard = module.contains(':');
        vec![Import {
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // D: import module; or import module : a, b, c;
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {};", import.module)
        } else {
            format!("import {} : {};", import.module, names_to_use.join(", "))
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        if text.starts_with("private ") {
            Visibility::Private
        } else if text.starts_with("protected ") {
            Visibility::Protected
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(&content[child.byte_range()]);
            }
            // func_declaration: name is inside func_declarator
            if child.kind() == "func_declarator" {
                let mut inner = child.walk();
                for grandchild in child.children(&mut inner) {
                    if grandchild.kind() == "identifier" {
                        return Some(&content[grandchild.byte_range()]);
                    }
                }
            }
        }
        None
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: DModuleResolver = DModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for D {}

impl crate::RefactorCodeGen for D {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        match ty {
            Some(t) => format!("{} {}", t, name),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}auto {} = {};\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let ret_type = match &spec.ret {
            GenReturn::Unit => "void".to_string(),
            GenReturn::Single(v) => format!("/* {} */", v),
            GenReturn::Tuple(vs) => format!("/* Tuple!({}) */", vs.join(", ")),
            GenReturn::Result(ok, _) => format!("/* {} */", ok),
        };
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} {}", ty, p.name),
                None => format!("auto {}", p.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    return {};", indent, v),
            GenReturn::Tuple(vs) => {
                format!("\n{}    return tuple({});", indent, vs.join(", "))
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
            GenReturn::Single(v) => format!("{}auto {} = {}({});\n", indent, v, name, args),
            GenReturn::Tuple(vs) => format!(
                "{}auto {} = {}({}); // TODO: unpack ({})\n",
                indent,
                vs.first().cloned().unwrap_or_default(),
                name,
                args,
                vs.join(", ")
            ),
            GenReturn::Result(ok, _) => format!("{}auto {} = {}({});\n", indent, ok, name, args),
        }
    }

    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        // D parameters are `Type name`; scan for the token preceding `name`.
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
    use super::D;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn d_fn_basic() {
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
            body_lines: vec!["auto result = n * 2;".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            D.render_function(&spec),
            "\nprivate /* result */ double(int n) {\n    auto result = n * 2;\n    return result;\n}\n"
        );
    }

    #[test]
    fn d_fn_void() {
        let spec = ExtractedFnSpec {
            name: "log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["writeln(x);".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            D.render_function(&spec),
            "\nprivate void log() {\n    writeln(x);\n}\n"
        );
    }

    #[test]
    fn d_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "compute".to_string(),
            params: vec![],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(D.render_call_site(&spec), "    auto result = compute();\n");
        assert_eq!(D.render_binding("x", "f()", "  "), "  auto x = f();\n");
        assert_eq!(D.format_param("n", Some("int")), "int n");
        assert_eq!(
            D.infer_param_type("int double(int n, string s)", "s"),
            Some("string".to_string())
        );
    }
}

// =============================================================================
// D Module Resolver
// =============================================================================

/// Module resolver for D.
///
/// Uses `dub.json` at the workspace root to find source paths (`sourcePaths`,
/// default `["source"]`). Module names map to file paths: `mypackage.utils` →
/// `mypackage/utils.d` under a source root.
pub struct DModuleResolver;

impl ModuleResolver for DModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut search_roots: Vec<PathBuf> = Vec::new();

        // Try dub.json first
        let dub_json = root.join("dub.json");
        if let Ok(content) = std::fs::read_to_string(&dub_json)
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(paths) = parsed.get("sourcePaths").and_then(|v| v.as_array())
        {
            for path in paths {
                if let Some(s) = path.as_str() {
                    search_roots.push(root.join(s));
                }
            }
        }

        // Default to source/ (dub convention) if not found in config
        if search_roots.is_empty() {
            search_roots.push(root.join("source"));
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots,
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "d" && ext != "di" {
            return Vec::new();
        }

        for root in &cfg.search_roots {
            if let Ok(rel) = file.strip_prefix(root) {
                let rel_str = rel.to_string_lossy();
                // Strip .d/.di extension and replace / with .
                let base = rel_str
                    .strip_suffix(".di")
                    .or_else(|| rel_str.strip_suffix(".d"))
                    .unwrap_or(&rel_str);
                let canonical = if cfg!(windows) {
                    base.replace('\\', ".")
                } else {
                    base.replace('/', ".")
                };
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
        if ext != "d" && ext != "di" {
            return Resolution::NotApplicable;
        }

        // `mypackage.utils` → `mypackage/utils.d`
        let file_path = spec.raw.replace('.', "/") + ".d";

        for root in &cfg.search_roots {
            let candidate = root.join(&file_path);
            if candidate.exists() {
                return Resolution::Resolved(candidate, String::new());
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
            // Expressions
            "add_expression", "and_and_expression", "and_expression", "assign_expression",
            "assert_expression", "cat_expression", "cast_expression", "comma_expression",
            "complement_expression", "conditional_expression", "delete_expression", "equal_expression",
            "expression", "identity_expression", "import_expression", "in_expression",
            "index_expression", "is_expression", "key_expression", "lwr_expression",
            "mixin_expression", "mul_expression", "new_anon_class_expression", "new_expression",
            "or_expression", "or_or_expression", "postfix_expression", "pow_expression",
            "primary_expression", "qualified_identifier", "rel_expression", "shift_expression",
            "slice_expression", "traits_expression", "typeid_expression", "unary_expression",
            "upr_expression", "value_expression", "xor_expression",
            // Statements
            "asm_statement", "break_statement", "case_range_statement", "case_statement",
            "conditional_statement", "continue_statement", "declaration_statement", "default_statement",
            "do_statement", "empty_statement", "expression_statement", "final_switch_statement",
            "foreach_range_statement", "goto_statement", "labeled_statement", "mixin_statement",
            "out_statement", "pragma_statement", "return_statement", "scope_block_statement",
            "scope_guard_statement", "scope_statement_list", "statement_list",
            "statement_list_no_case_no_default", "static_foreach_statement", "synchronized_statement",
            "then_statement", "throw_statement", "try_statement", "with_statement",
            // Declarations
            "anonymous_enum_declaration", "anonymous_enum_member",
            "anonymous_enum_members", "anon_struct_declaration", "anon_union_declaration",
            "auto_func_declaration", "class_template_declaration",
            "conditional_declaration", "debug_specification", "destructor", "empty_declaration",
            "enum_body", "enum_member", "enum_member_attribute", "enum_member_attributes",
            "enum_members", "interface_template_declaration", "mixin_declaration",
            "module", "shared_static_constructor", "shared_static_destructor", "static_constructor",
            "static_destructor", "static_foreach_declaration", "struct_template_declaration",
            "template_declaration", "template_mixin_declaration", "union_declaration",
            "union_template_declaration", "var_declarations", "version_specification",
            // Foreach-related
            "aggregate_foreach", "foreach", "foreach_aggregate", "foreach_type",
            "foreach_type_attribute", "foreach_type_attributes", "foreach_type_list",
            "range_foreach", "static_foreach",
            // Function-related
            "constructor_args", "constructor_template", "function_attribute_kwd",
            "function_attributes", "function_contracts", "function_literal_body",
            "function_literal_body2", "member_function_attribute", "member_function_attributes",
            "missing_function_body", "out_contract_expression", "in_contract_expression",
            "in_statement", "parameter_with_attributes", "parameter_with_member_attributes",
            "shortened_function_body", "specified_function_body",
            // Template-related
            "template_type_parameter", "template_type_parameter_default",
            "template_type_parameter_specialization", "type_specialization",
            // Type-related
            "aggregate_body", "basic_type", "catch_parameter", "catches", "constructor",
            "else_statement", "enum_base_type", "finally_statement", "fundamental_type",
            "if_condition", "interfaces", "linkage_type", "module_alias_identifier",
            "module_attributes", "module_fully_qualified_name", "module_name", "mixin_type",
            "mixin_qualified_identifier", "storage_class", "storage_classes", "type",
            "type_ctor", "type_ctors", "type_suffix", "type_suffixes", "typeof", "interface",
            // Import-related
            "import", "import_bind", "import_bind_list", "import_bindings", "import_list",
            // ASM-related
            "asm_instruction", "asm_instruction_list", "asm_shift_exp", "asm_type_prefix",
            "gcc_asm_instruction_list", "gcc_asm_statement", "gcc_basic_asm_instruction",
            "gcc_ext_asm_instruction", "gcc_goto_asm_instruction",
            // Misc
            "alt_declarator_identifier", "base_class_list", "base_interface_list",
            "block_comment", "declaration_block", "declarator_identifier_list", "dot_identifier", "nesting_block_comment", "static_if_condition", "struct_initializer",
            "struct_member_initializer", "struct_member_initializers", "super_class_or_interface",
            "traits_arguments", "traits_keyword", "var_declarator_identifier", "vector_base_type",
            "attribute_specifier",
            // structural node, not extracted as symbols
            "alias_declaration",
            "auto_declaration",
            "module_declaration",
            "block_statement",
            "import_declaration",
            "while_statement",
            "switch_statement",
            "if_statement",
            "function_literal",
            "for_statement",
            "foreach_statement",
            "catch",
        ];
        validate_unused_kinds_audit(&D, documented_unused)
            .expect("D unused node kinds audit failed");
    }
}
