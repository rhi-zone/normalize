//! Python language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

// ============================================================================
// Python language support
// ============================================================================

/// Python language support.
pub struct Python;

impl Language for Python {
    fn name(&self) -> &'static str {
        "Python"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi", "pyw"]
    }
    fn grammar_name(&self) -> &'static str {
        "python"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["class_definition"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["class_definition"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import_statement", "import_from_statement"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "class_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
            "except_clause",
            "with_statement",
            "match_statement",
            "case_clause",
            "and",
            "or",
            "conditional_expression",
            "list_comprehension",
            "dictionary_comprehension",
            "set_comprehension",
            "generator_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
            "with_statement",
            "match_statement",
            "function_definition",
            "class_definition",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        // Additional scope-creating nodes beyond functions and containers
        &[
            "for_statement",
            "with_statement",
            "list_comprehension",
            "set_comprehension",
            "dictionary_comprehension",
            "generator_expression",
            "lambda",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
            "with_statement",
            "match_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "raise_statement",
            "assert_statement",
        ]
    }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        // Skip private methods unless they're dunder methods
        // (visibility filtering can be done by caller)

        // Check for async keyword as first child token
        let is_async = node
            .child(0)
            .map(|c| &content[c.byte_range()] == "async")
            .unwrap_or(false);
        let prefix = if is_async { "async def" } else { "def" };

        let params = node
            .child_by_field_name("parameters")
            .map(|p| &content[p.byte_range()])
            .unwrap_or("()");

        let return_type = node
            .child_by_field_name("return_type")
            .map(|r| format!(" -> {}", &content[r.byte_range()]))
            .unwrap_or_default();

        let signature = format!("{} {}{}{}", prefix, name, params, return_type);
        let visibility = self.get_visibility(node, content);

        Some(Symbol {
            name: name.to_string(),
            kind: if in_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            },
            signature,
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let bases = node
            .child_by_field_name("superclasses")
            .map(|b| &content[b.byte_range()])
            .unwrap_or("");

        let signature = if bases.is_empty() {
            format!("class {}", name)
        } else {
            format!("class {}{}", name, bases)
        };

        // Extract superclasses from argument_list children
        let mut implements = Vec::new();
        if let Some(superclasses) = node.child_by_field_name("superclasses") {
            let mut cursor = superclasses.walk();
            for child in superclasses.children(&mut cursor) {
                if child.kind() == "identifier" {
                    implements.push(content[child.byte_range()].to_string());
                }
            }
        }

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Class,
            signature,
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(), // Caller fills this in
            is_interface_impl: false,
            implements,
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        // Python classes are both containers and types
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let body = node.child_by_field_name("body")?;
        let first = body.child(0)?;

        // Handle both grammar versions:
        // - Old: expression_statement > string
        // - New (arborium): string directly, with string_content child
        let string_node = match first.kind() {
            "string" => Some(first),
            "expression_statement" => first.child(0).filter(|n| n.kind() == "string"),
            _ => None,
        }?;

        // Try string_content child (arborium style)
        let mut cursor = string_node.walk();
        for child in string_node.children(&mut cursor) {
            if child.kind() == "string_content" {
                let doc = content[child.byte_range()].trim();
                if !doc.is_empty() {
                    return Some(doc.to_string());
                }
            }
        }

        // Fallback: extract from full string text (old style)
        let text = &content[string_node.byte_range()];
        let doc = text
            .trim_start_matches("\"\"\"")
            .trim_start_matches("'''")
            .trim_start_matches('"')
            .trim_start_matches('\'')
            .trim_end_matches("\"\"\"")
            .trim_end_matches("'''")
            .trim_end_matches('"')
            .trim_end_matches('\'')
            .trim();

        if !doc.is_empty() {
            Some(doc.to_string())
        } else {
            None
        }
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let line = node.start_position().row + 1;

        match node.kind() {
            "import_statement" => {
                // import foo, import foo as bar
                let mut imports = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        let module = content[child.byte_range()].to_string();
                        imports.push(Import {
                            module,
                            names: Vec::new(),
                            alias: None,
                            is_wildcard: false,
                            is_relative: false,
                            line,
                        });
                    } else if child.kind() == "aliased_import"
                        && let Some(name) = child.child_by_field_name("name")
                    {
                        let module = content[name.byte_range()].to_string();
                        let alias = child
                            .child_by_field_name("alias")
                            .map(|a| content[a.byte_range()].to_string());
                        imports.push(Import {
                            module,
                            names: Vec::new(),
                            alias,
                            is_wildcard: false,
                            is_relative: false,
                            line,
                        });
                    }
                }
                imports
            }
            "import_from_statement" => {
                // from foo import bar, baz
                let module = node
                    .child_by_field_name("module_name")
                    .map(|m| content[m.byte_range()].to_string())
                    .unwrap_or_default();

                // Check for relative import (from . or from .. or from .foo)
                let text = &content[node.byte_range()];
                let is_relative = text.starts_with("from .");

                let mut names = Vec::new();
                let mut is_wildcard = false;
                let module_end = node
                    .child_by_field_name("module_name")
                    .map(|m| m.end_byte())
                    .unwrap_or(0);

                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "dotted_name" | "identifier" => {
                            // Skip the module name itself
                            if child.start_byte() > module_end {
                                names.push(content[child.byte_range()].to_string());
                            }
                        }
                        "aliased_import" => {
                            if let Some(name) = child.child_by_field_name("name") {
                                names.push(content[name.byte_range()].to_string());
                            }
                        }
                        "wildcard_import" => {
                            is_wildcard = true;
                        }
                        _ => {}
                    }
                }

                vec![Import {
                    module,
                    names,
                    alias: None,
                    is_wildcard,
                    is_relative,
                    line,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());

        if import.is_wildcard {
            format!("from {} import *", import.module)
        } else if names_to_use.is_empty() {
            if let Some(ref alias) = import.alias {
                format!("import {} as {}", import.module, alias)
            } else {
                format!("import {}", import.module)
            }
        } else {
            format!("from {} import {}", import.module, names_to_use.join(", "))
        }
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let line = node.start_position().row + 1;

        match node.kind() {
            "function_definition" => {
                if let Some(name) = self.node_name(node, content)
                    && !name.starts_with('_')
                {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line,
                    }];
                }
                Vec::new()
            }
            "class_definition" => {
                if let Some(name) = self.node_name(node, content)
                    && !name.starts_with('_')
                {
                    return vec![Export {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        line,
                    }];
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        if let Some(name) = self.node_name(node, content) {
            // Public if doesn't start with _ or is dunder method
            !name.starts_with('_') || name.starts_with("__")
        } else {
            true
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if let Some(name) = self.node_name(node, content) {
            if name.starts_with("__") && name.ends_with("__") {
                Visibility::Public // dunder methods
            } else if name.starts_with("__") {
                Visibility::Private // name mangled
            } else if name.starts_with('_') {
                Visibility::Protected // convention private
            } else {
                Visibility::Public
            }
        } else {
            Visibility::Public
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Class => name.starts_with("Test") && name.len() > 4,
            crate::SymbolKind::Module => name == "tests" || name == "test" || name == "__tests__",
            _ => false,
        }
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn body_has_docstring(&self, body: &Node, content: &str) -> bool {
        let _ = content;
        body.child(0)
            .map(|c| {
                c.kind() == "string"
                    || (c.kind() == "expression_statement"
                        && c.child(0).map(|n| n.kind() == "string").unwrap_or(false))
            })
            .unwrap_or(false)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrammarLoader;
    use tree_sitter::Parser;

    struct ParseResult {
        tree: tree_sitter::Tree,
        #[allow(dead_code)]
        loader: GrammarLoader,
    }

    fn parse_python(content: &str) -> ParseResult {
        let loader = GrammarLoader::new();
        let language = loader.get("python").unwrap();
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        ParseResult {
            tree: parser.parse(content, None).unwrap(),
            loader,
        }
    }

    #[test]
    fn test_python_function_kinds() {
        let support = Python;
        assert!(support.function_kinds().contains(&"function_definition"));
        // async functions are function_definition with "async" keyword as first child
    }

    #[test]
    fn test_python_extract_function() {
        let support = Python;
        let content = r#"def foo(x: int) -> str:
    """Convert to string."""
    return str(x)
"#;
        let result = parse_python(content);
        let root = result.tree.root_node();

        // Find function node
        let mut cursor = root.walk();
        let func = root
            .children(&mut cursor)
            .find(|n| n.kind() == "function_definition")
            .unwrap();

        let sym = support.extract_function(&func, content, false).unwrap();
        assert_eq!(sym.name, "foo");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.signature.contains("def foo(x: int) -> str"));
        assert_eq!(sym.docstring, Some("Convert to string.".to_string()));
    }

    #[test]
    fn test_python_extract_class() {
        let support = Python;
        let content = r#"class Foo(Bar):
    """A foo class."""
    pass
"#;
        let result = parse_python(content);
        let root = result.tree.root_node();

        let mut cursor = root.walk();
        let class = root
            .children(&mut cursor)
            .find(|n| n.kind() == "class_definition")
            .unwrap();

        let sym = support.extract_container(&class, content).unwrap();
        assert_eq!(sym.name, "Foo");
        assert_eq!(sym.kind, SymbolKind::Class);
        assert!(sym.signature.contains("class Foo(Bar)"));
        assert_eq!(sym.docstring, Some("A foo class.".to_string()));
    }

    #[test]
    fn test_python_visibility() {
        let support = Python;
        let content = r#"def public(): pass
def _protected(): pass
def __private(): pass
def __dunder__(): pass
"#;
        let result = parse_python(content);
        let root = result.tree.root_node();

        let mut cursor = root.walk();
        let funcs: Vec<_> = root
            .children(&mut cursor)
            .filter(|n| n.kind() == "function_definition")
            .collect();

        assert_eq!(
            support.get_visibility(&funcs[0], content),
            Visibility::Public
        );
        assert_eq!(
            support.get_visibility(&funcs[1], content),
            Visibility::Protected
        );
        assert_eq!(
            support.get_visibility(&funcs[2], content),
            Visibility::Private
        );
        assert_eq!(
            support.get_visibility(&funcs[3], content),
            Visibility::Public
        ); // dunder
    }

    /// Documents node kinds that exist in the Python grammar but aren't used in trait methods.
    /// Each exclusion has a reason. Review periodically as features expand.
    ///
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        use crate::validate_unused_kinds_audit;

        // Categories:
        // - STRUCTURAL: Internal/wrapper nodes, not semantically meaningful on their own
        // - CLAUSE: Sub-parts of statements, handled via parent (e.g., else_clause in if_statement)
        // - EXPRESSION: Expressions don't create control flow/scope, we track statements
        // - TYPE: Type annotation nodes, not relevant for current analysis
        // - LEGACY: Python 2 compatibility, not worth supporting
        // - OPERATOR: Operators within expressions, too granular
        // - TODO: Potentially useful, to be added when needed

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "aliased_import",          // used internally by extract_imports
            "block",                   // generic block wrapper (duplicate in grammar)
            "expression_list",         // comma-separated expressions
            "identifier",              // too common, used everywhere
            "import_prefix",           // dots in relative imports
            "lambda_parameters",       // internal to lambda
            "module",                  // root node of file
            "parenthesized_expression",// grouping only
            "relative_import",         // handled in extract_imports
            "tuple_expression",        // comma-separated values
            "wildcard_import",         // handled in extract_imports

            // CLAUSE (sub-parts of statements)
            "case_pattern",            // internal to case_clause
            "class_pattern",           // pattern in match/case
            "elif_clause",             // part of if_statement
            "else_clause",             // part of if/for/while/try
            "finally_clause",          // part of try_statement
            "for_in_clause",           // internal to comprehensions
            "if_clause",               // internal to comprehensions
            "with_clause",             // internal to with_statement
            "with_item",               // internal to with_statement

            // EXPRESSION (don't affect control flow structure)
            "await",                   // await keyword, not a statement
            "format_expression",       // f-string interpolation
            "format_specifier",        // f-string format spec
            "named_expression",        // walrus operator :=
            "yield",                   // yield keyword form

            // TYPE (type annotations)
            "constrained_type",        // type constraints
            "generic_type",            // parameterized types
            "member_type",             // attribute access in types
            "splat_type",              // *args/**kwargs types
            "type",                    // generic type node
            "type_alias_statement",    // could track as symbol
            "type_conversion",         // !r/!s/!a in f-strings
            "type_parameter",          // generic type params
            "typed_default_parameter", // param with type and default
            "typed_parameter",         // param with type annotation
            "union_type",              // X | Y union syntax

            // OPERATOR
            "binary_operator",         // +, -, *, /, etc.
            "boolean_operator",        // and/or - handled in complexity_nodes as keywords
            "comparison_operator",     // ==, <, >, etc.
            "not_operator",            // not keyword
            "unary_operator",          // -, +, ~

            // LEGACY (Python 2)
            "exec_statement",          // Python 2 exec
            "print_statement",         // Python 2 print

            // TODO: Potentially useful
            "decorated_definition",    // wrapper for @decorator
            "delete_statement",        // del statement
            "future_import_statement", // from __future__
            "global_statement",        // scope modifier
            "nonlocal_statement",      // scope modifier
            "pass_statement",          // no-op, detect empty bodies
        ];

        validate_unused_kinds_audit(&Python, documented_unused)
            .expect("Python unused node kinds audit failed");
    }
}
