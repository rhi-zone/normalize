//! JavaScript language support.

use crate::ecmascript;
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
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

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        ecmascript::extract_decorators(node, content)
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
        ecmascript::get_visibility(node, content)
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        ecmascript::extract_js_module_doc(src)
    }
}

impl LanguageSymbols for JavaScript {}

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
            // control flow — not extracted as symbols
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
