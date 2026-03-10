//! C++ language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use tree_sitter::Node;

/// C++ language support.
pub struct Cpp;

impl Language for Cpp {
    fn name(&self) -> &'static str {
        "C++"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cc", "cxx", "hpp", "hh", "hxx"]
    }
    fn grammar_name(&self) -> &'static str {
        "cpp"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
                let text = &content[sibling.byte_range()];
                if text.starts_with("/**") {
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
                }
                return None;
            }
            if sibling.kind() == "template_declaration" {
                prev = sibling.prev_sibling();
                continue;
            }
            return None;
        }
        None
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_clause" {
                let mut bc = child.walk();
                for base in child.children(&mut bc) {
                    if base.kind() == "type_identifier" {
                        implements.push(content[base.byte_range()].to_string());
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
        match node.kind() {
            "function_definition" => {
                if let Some(declarator) = node.child_by_field_name("declarator")
                    && let Some(name) = find_identifier(&declarator, content)
                {
                    return name.to_string();
                }
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
            "class_specifier" => {
                let name = self.node_name(node, content).unwrap_or("");
                format!("class {}", name)
            }
            "struct_specifier" => {
                let name = self.node_name(node, content).unwrap_or("");
                format!("struct {}", name)
            }
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "preproc_include" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string_literal" || child.kind() == "system_lib_string" {
                let text = &content[child.byte_range()];
                let module = text
                    .trim_matches(|c| c == '"' || c == '<' || c == '>')
                    .to_string();
                let is_relative = text.starts_with('"');
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative,
                    line,
                }];
            }
        }
        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // C++ uses #include, no multi-imports
        if import.module.starts_with('<') || import.module.ends_with('>') {
            format!("#include {}", import.module)
        } else {
            format!("#include \"{}\"", import.module)
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
        &[
            "**/test_*.cpp",
            "**/*_test.cpp",
            "**/test_*.cc",
            "**/*_test.cc",
            "**/tests/**/*.cpp",
            "**/tests/**/*.cc",
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
        // C++ visibility is determined by the most recent access_specifier in the enclosing
        // class body. Classes default to private, structs to public.
        // Walk backward through siblings to find the nearest access_specifier.
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "access_specifier" {
                let spec = content[sibling.byte_range()].trim().trim_end_matches(':');
                return match spec {
                    "public" => Visibility::Public,
                    "protected" => Visibility::Protected,
                    "private" => Visibility::Private,
                    _ => Visibility::Public,
                };
            }
            prev = sibling.prev_sibling();
        }
        // No access_specifier found: check if parent is struct (public) or class (private).
        // If we can't determine, default to Public (safe for analysis).
        if node
            .parent()
            .and_then(|p| p.parent())
            .map(|g| g.kind() == "class_specifier")
            .unwrap_or(false)
        {
            return Visibility::Private;
        }
        Visibility::Public
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(&content[name_node.byte_range()]);
        }
        if let Some(declarator) = node.child_by_field_name("declarator") {
            return find_identifier(&declarator, content);
        }
        None
    }
}

impl LanguageSymbols for Cpp {}

fn find_identifier<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    if node.kind() == "identifier" || node.kind() == "field_identifier" {
        return Some(&content[node.byte_range()]);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(id) = find_identifier(&child, content) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    /// Documents node kinds that exist in the C++ grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL (C++ adds many to C)
            "access_specifier",        // public:, private:
            "base_class_clause",       // : public Base
            "bitfield_clause",         // : width
            "condition_clause",        // if condition             // declaration
            "declaration_list",        // decl list
            "default_method_clause",   // = default
            "delete_method_clause",    // = delete
            "dependent_type",          // typename T::X
            "destructor_name",         // ~Foo
            "enumerator",              // enum value
            "enumerator_list",         // enum body
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_expression",        // foo.bar
            "field_identifier",        // field name
            "identifier",              // too common
            "init_statement",          // for init
            "linkage_specification",   // extern "C"
            "module_name",             // module name
            "module_partition",        // module partition
            "namespace_identifier",    // namespace name
            "nested_namespace_specifier", // ns1::ns2
            "operator_name",           // operator+
            "parameter_declaration",   // param decl
            "primitive_type",          // int, char
            "pure_virtual_clause",     // = 0
            "ref_qualifier",           // &, &&
            "sized_type_specifier",    // unsigned int
            "statement_identifier",    // label name
            "static_assert_declaration", // static_assert
            "storage_class_specifier", // static, extern
            "structured_binding_declarator", // auto [a, b]
            "type_descriptor",         // type desc
            "type_identifier",         // type name
            "type_parameter_declaration", // template param
            "type_qualifier",          // const, volatile
            "union_specifier",         // union
            "using_declaration",       // using ns::name
            "variadic_parameter_declaration", // T...
            "variadic_type_parameter_declaration", // typename...
            "virtual_specifier",       // override, final

            // CLAUSE
            "else_clause",             // else
            "noexcept",                // noexcept

            // EXPRESSION (C++ adds many to C)
            "alignof_expression",      // alignof(T)
            "assignment_expression",   // x = y
            "binary_expression",       // a + b
            "call_expression",         // foo()
            "cast_expression",         // (T)x
            "co_await_expression",     // co_await x
            "co_return_statement",     // co_return
            "co_yield_statement",      // co_yield x
            "comma_expression",        // a, b
            "compound_literal_expression", // (T){...}
            "delete_expression",       // delete x
            "extension_expression",    // __extension__
            "fold_expression",         // (... + args)
            "generic_expression",      // _Generic
            "gnu_asm_expression",      // asm()
            "new_expression",          // new T
            "offsetof_expression",     // offsetof
            "parenthesized_expression",// (expr)
            "pointer_expression",      // *p, &x
            "reflect_expression",      // reflexpr
            "sizeof_expression",       // sizeof(T)
            "splice_expression",       // [:expr:]
            "subscript_expression",    // arr[i]
            "unary_expression",        // -x, !x
            "update_expression",       // x++

            // TEMPLATE
            "template_declaration",    // template<>
            "template_function",       // template func
            "template_method",         // template method
            "template_template_parameter_declaration", // template template
            "template_type",           // T<U>

            // LAMBDA
            "lambda_capture_initializer", // [x = y]
            "lambda_capture_specifier",   // [=], [&]
            "lambda_declarator",       // lambda params
            "lambda_default_capture",  // =, &
            "lambda_specifier",        // mutable

            // FUNCTION
            "abstract_function_declarator", // abstract func
            "explicit_function_specifier", // explicit
            "explicit_object_parameter_declaration", // this param     // func decl
            "operator_cast",           // operator T()
            "optional_parameter_declaration", // param = default
            "optional_type_parameter_declaration", // T = U
            "placeholder_type_specifier", // auto, decltype(auto)
            "pointer_type_declarator", // ptr declarator
            "trailing_return_type",    // -> T

            // CONCEPTS/REQUIRES
            "concept_definition",      // concept
            "requires_clause",         // requires
            "requires_expression",     // requires {}
            "type_requirement",        // typename T

            // MODULE
            "export_declaration",      // export
            "global_module_fragment_declaration", // module;
            "import_declaration",      // import
            "module_declaration",      // module
            "private_module_fragment_declaration", // module :private

            // PREPROCESSOR
            "preproc_elif",            // #elif
            "preproc_elifdef",         // #elifdef
            "preproc_else",            // #else
            "preproc_function_def",    // function macro
            "preproc_if",              // #if
            "preproc_ifdef",           // #ifdef

            // SPLICE
            "splice_specifier",        // [: :] specifier
            "splice_type_specifier",   // [: :] type

            // OTHER
            "alias_declaration",       // using X = Y
            "alignas_qualifier",       // alignas
            "attribute_declaration",   // [[attr]]
            "attribute_specifier",     // __attribute__
            "attributed_statement",    // stmt with attr
            "consteval_block_declaration", // consteval
            "decltype",                // decltype
            "expansion_statement",     // pack expansion stmt
            "expression_statement",    // expr;
            "friend_declaration",      // friend
            "gnu_asm_qualifier",       // asm qualifiers
            "labeled_statement",       // label:
            "namespace_alias_definition", // namespace X = Y
            "qualified_identifier",    // ns::name
            "throw_specifier",         // throw()

            // MS EXTENSIONS
            "ms_based_modifier",       // __based
            "ms_call_modifier",        // __cdecl
            "ms_declspec_modifier",    // __declspec
            "ms_pointer_modifier",     // __ptr32
            "ms_restrict_modifier",    // __restrict
            "ms_signed_ptr_modifier",  // __sptr
            "ms_unaligned_ptr_modifier", // __unaligned
            "ms_unsigned_ptr_modifier", // __uptr

            // SEH
            "seh_except_clause",       // __except
            "seh_finally_clause",      // __finally
            "seh_leave_statement",     // __leave
            "seh_try_statement",       // __try
            // covered by tags.scm
            "function_definition",
            "case_statement",
            "for_range_loop",
            "conditional_expression",
            "do_statement",
            "if_statement",
            "catch_clause",
            "while_statement",
            "lambda_expression",
            "continue_statement",
            "switch_statement",
            "throw_statement",
            "try_statement",
            "return_statement",
            "break_statement",
            "compound_statement",
            "namespace_definition",
            "goto_statement",
            "for_statement",
        ];

        validate_unused_kinds_audit(&Cpp, documented_unused)
            .expect("C++ unused node kinds audit failed");
    }
}
