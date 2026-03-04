//! JavaScript language support.

use crate::ecmascript;
use crate::{ContainerBody, Import, Language, Symbol, Visibility};
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

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        ecmascript::format_import(import, names)
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

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/__tests__/**/*.js",
            "**/__mocks__/**/*.js",
            "**/*.test.js",
            "**/*.spec.js",
            "**/*.test.jsx",
            "**/*.spec.jsx",
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
            "field_definition",        // class field              // too common
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

            // EXPRESSION   // x = y
            "augmented_assignment_expression", // x += y
            "await_expression",        // await foo         // foo()     // function() {}       // foo.bar          // new Foo()
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
            "expression_statement",    // expr;      // function* foo
            "labeled_statement",       // label: stmt     // let/const
            "using_declaration",       // using x = ...    // var x
            "with_statement",          // with (obj) - deprecated

            // JSX
            "jsx_expression",          // {expr} in JSX
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "break_statement",
            "while_statement",
            "throw_statement",
            "if_statement",
            "for_statement",
            "import_statement",
            "ternary_expression",
            "catch_clause",
            "do_statement",
            "return_statement",
            "try_statement",
            "for_in_statement",
            "continue_statement",
            "switch_statement",
            "switch_case",
            "arrow_function",
        ];

        validate_unused_kinds_audit(&JavaScript, documented_unused)
            .expect("JavaScript unused node kinds audit failed");
    }
}
