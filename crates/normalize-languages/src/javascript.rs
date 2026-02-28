//! JavaScript language support.

use crate::ecmascript;
use crate::{ContainerBody, Export, Import, Language, Symbol, Visibility, VisibilityMechanism};
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

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        ecmascript::JS_CONTAINER_KINDS
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        ecmascript::JS_FUNCTION_KINDS
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        ecmascript::JS_TYPE_KINDS
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
        // JS classes are the only type-like construct
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        // JS doesn't have standardized docstrings (JSDoc would require comment parsing)
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
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

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        // JS uses export statements, not visibility modifiers on declarations
        true
    }

    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
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
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
    }
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
            "field_definition",        // class field
            "identifier",              // too common
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

            // EXPRESSION
            "assignment_expression",   // x = y
            "augmented_assignment_expression", // x += y
            "await_expression",        // await foo
            "call_expression",         // foo()
            "function_expression",     // function() {}
            "member_expression",       // foo.bar
            "new_expression",          // new Foo()
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
            "expression_statement",    // expr;
            "generator_function",      // function* foo
            "labeled_statement",       // label: stmt
            "lexical_declaration",     // let/const
            "using_declaration",       // using x = ...
            "variable_declaration",    // var x
            "with_statement",          // with (obj) - deprecated

            // JSX
            "jsx_expression",          // {expr} in JSX
        ];

        validate_unused_kinds_audit(&JavaScript, documented_unused)
            .expect("JavaScript unused node kinds audit failed");
    }
}
