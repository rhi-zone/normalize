//! TypeScript language support.

use crate::ecmascript;
use crate::{Export, Import, Language, Symbol, Visibility, VisibilityMechanism};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_CONTAINER_KINDS
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_FUNCTION_KINDS
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_TYPE_KINDS
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        ecmascript::IMPORT_KINDS
    }
    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        ecmascript::PUBLIC_SYMBOL_KINDS
    }
    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        ecmascript::SCOPE_CREATING_KINDS
    }
    fn control_flow_kinds(&self) -> &'static [&'static str] {
        ecmascript::CONTROL_FLOW_KINDS
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        ecmascript::COMPLEXITY_NODES
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        ecmascript::NESTING_NODES
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_function(
            node,
            content,
            in_container,
            name,
        ))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_container(node, content, name))
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        ecmascript::extract_type(node, name)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        ecmascript::format_import(import, names)
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        ecmascript::extract_public_symbols(node, content)
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }

    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
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

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_CONTAINER_KINDS
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_FUNCTION_KINDS
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        ecmascript::TS_TYPE_KINDS
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        ecmascript::IMPORT_KINDS
    }
    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        ecmascript::PUBLIC_SYMBOL_KINDS
    }
    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }
    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        ecmascript::SCOPE_CREATING_KINDS
    }
    fn control_flow_kinds(&self) -> &'static [&'static str] {
        ecmascript::CONTROL_FLOW_KINDS
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        ecmascript::COMPLEXITY_NODES
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        ecmascript::NESTING_NODES
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_function(
            node,
            content,
            in_container,
            name,
        ))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_container(node, content, name))
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        ecmascript::extract_type(node, name)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        ecmascript::format_import(import, names)
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        ecmascript::extract_public_symbols(node, content)
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }

    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
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

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
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
            "member_expression",       // foo.bar
            "new_expression",          // new Foo()
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
            "flow_maybe_type",         // ?T
            "function_signature",      // function sig
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
            "tuple_type",              // [T, U]
            "type_annotation",         // : T
            "type_arguments",          // <T, U>
            "type_assertion",          // <T>x
            "type_identifier",         // type name
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

            // DECLARATION
            "abstract_class_declaration", // abstract class
            "abstract_method_signature", // abstract method
            "ambient_declaration",     // declare
            "debugger_statement",      // debugger;
            "empty_statement",         // ;
            "expression_statement",    // expr;
            "generator_function",      // function* foo
            "generator_function_declaration", // function* declaration
            "internal_module",         // namespace/module
            "labeled_statement",       // label: stmt
            "lexical_declaration",     // let/const
            "module",                  // module keyword
            "using_declaration",       // using x = ...
            "variable_declaration",    // var x
            "with_statement",          // with (obj) - deprecated
        ];

        validate_unused_kinds_audit(&TypeScript, documented_unused)
            .expect("TypeScript unused node kinds audit failed");
    }
}
