//! TypeScript language support.

use crate::ecmascript;
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
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
}

impl LanguageSymbols for TypeScript {}

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
