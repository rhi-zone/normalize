//! Go language support.

use crate::{ContainerBody, Import, Language, Visibility};
use tree_sitter::Node;

/// Go language support.
pub struct Go;

impl Language for Go {
    fn name(&self) -> &'static str {
        "Go"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }
    fn grammar_name(&self) -> &'static str {
        "go"
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
                if let Some(line) = text.strip_prefix("//") {
                    let line = line.strip_prefix(' ').unwrap_or(line);
                    doc_lines.push(line.to_string());
                } else {
                    break;
                }
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        let joined = doc_lines.join("\n").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
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
            "function_declaration" | "method_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                format!("func {}{}", name, params)
            }
            "type_spec" => format!("type {}", name),
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

        let mut imports = Vec::new();
        let line = node.start_position().row + 1;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "import_spec" => {
                    // import "path" or import alias "path"
                    if let Some(imp) = Self::parse_import_spec(&child, content, line) {
                        imports.push(imp);
                    }
                }
                "import_spec_list" => {
                    // Grouped imports
                    let mut list_cursor = child.walk();
                    for spec in child.children(&mut list_cursor) {
                        if spec.kind() == "import_spec"
                            && let Some(imp) = Self::parse_import_spec(&spec, content, line)
                        {
                            imports.push(imp);
                        }
                    }
                }
                _ => {}
            }
        }

        imports
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Go: import "pkg" or import alias "pkg"
        if let Some(ref alias) = import.alias {
            format!("import {} \"{}\"", alias, import.module)
        } else {
            format!("import \"{}\"", import.module)
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let is_exported = self
            .node_name(node, content)
            .and_then(|n| n.chars().next())
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        if is_exported {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        match symbol.kind {
            crate::SymbolKind::Function => {
                let name = symbol.name.as_str();
                name.starts_with("Test")
                    || name.starts_with("Benchmark")
                    || name.starts_with("Example")
            }
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/*_test.go"]
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

impl Go {
    fn parse_import_spec(node: &Node, content: &str, line: usize) -> Option<Import> {
        let mut path = String::new();
        let mut alias = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "interpreted_string_literal" => {
                    let text = &content[child.byte_range()];
                    path = text.trim_matches('"').to_string();
                }
                "package_identifier" | "blank_identifier" | "dot" => {
                    alias = Some(content[child.byte_range()].to_string());
                }
                _ => {}
            }
        }

        if path.is_empty() {
            return None;
        }

        let is_wildcard = alias.as_deref() == Some(".");
        Some(Import {
            module: path,
            names: Vec::new(),
            alias,
            is_wildcard,
            is_relative: false, // Go doesn't have relative imports in the traditional sense
            line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents node kinds that exist in the Go grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        use crate::validate_unused_kinds_audit;

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "blank_identifier",        // _
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_identifier",        // field name              // too common          // package foo
            "package_identifier",      // package name
            "parameter_declaration",   // func param
            "statement_list",          // block contents
            "variadic_parameter_declaration", // ...T

            // CLAUSE
            "default_case",            // default:
            "for_clause",              // for init; cond; post
            "import_spec",             // import spec
            "import_spec_list",        // import block
            "method_elem",             // interface method
            "range_clause",            // for range

            // EXPRESSION         // foo()
            "index_expression",        // arr[i]// (expr)     // foo.bar
            "slice_expression",        // arr[1:3]
            "type_assertion_expression", // x.(T)
            "type_conversion_expression", // T(x)
            "type_instantiation_expression", // generic instantiation
            "unary_expression",        // -x, !x

            // TYPE
            "array_type",              // [N]T
            "channel_type",            // chan T
            "implicit_length_array_type", // [...]T
            "function_type",           // func(T) U
            "generic_type",            // T[U]
            "interface_type",          // interface{}
            "map_type",                // map[K]V
            "negated_type",            // ~T
            "parenthesized_type",      // (T)
            "pointer_type",            // *T
            "qualified_type",          // pkg.Type
            "slice_type",              // []T
            "struct_type",             // struct{}
            "type_arguments",          // [T, U]
            "type_constraint",         // T constraint
            "type_elem",               // type element         // type name
            "type_parameter_declaration", // [T any]
            "type_parameter_list",     // type params

            // DECLARATION
            "assignment_statement",    // x = y       // const x = 1
            "dec_statement",           // x--
            "expression_list",         // a, b, c
            "expression_statement",    // expr
            "inc_statement",           // x++
            "short_var_declaration",   // x := y
            "type_alias",              // type X = Y        // type X struct{}         // var x int

            // CONTROL FLOW DETAILS
            "empty_statement",         // ;
            "fallthrough_statement",   // fallthrough
            "go_statement",            // go foo()
            "labeled_statement",       // label:
            "receive_statement",       // <-ch
            "send_statement",          // ch <- x
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "return_statement",
            "continue_statement",
            "break_statement",
            "if_statement",
            "for_statement",
            "goto_statement",
            "expression_switch_statement",
            "expression_case",
            "type_case",
            "type_switch_statement",
            "select_statement",
            "block",
            "defer_statement",
            "binary_expression",
            "communication_case",
        ];

        validate_unused_kinds_audit(&Go, documented_unused)
            .expect("Go unused node kinds audit failed");
    }
}
