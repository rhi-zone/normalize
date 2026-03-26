//! Go language support.

use crate::docstring::extract_preceding_prefix_comments;
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
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

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_preceding_prefix_comments(node, content, "//")
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        // Go type_spec wraps the actual type (struct_type, interface_type, etc.)
        if node.kind() == "type_spec"
            && let Some(type_node) = node.child_by_field_name("type")
        {
            return match type_node.kind() {
                "struct_type" => crate::SymbolKind::Struct,
                "interface_type" => crate::SymbolKind::Interface,
                _ => tag_kind,
            };
        }
        tag_kind
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

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        extract_go_package_doc(src)
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

impl LanguageSymbols for Go {}

/// Extract the Go package comment from source.
///
/// The Go convention is a block of `//` comments immediately before
/// the `package` keyword. Scans backwards from the `package` line.
/// A blank line between the comment and `package` means it is NOT a doc comment.
fn extract_go_package_doc(src: &str) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    // Find the package declaration line
    let pkg_idx = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with("package ") || t == "package"
    })?;

    // A blank line immediately before package means no doc comment
    if pkg_idx > 0 && lines[pkg_idx - 1].trim().is_empty() {
        return None;
    }

    // Collect comment lines immediately preceding the package line
    let mut doc_lines: Vec<&str> = Vec::new();
    let mut idx = pkg_idx;
    while idx > 0 {
        idx -= 1;
        let t = lines[idx].trim();
        if t.starts_with("//") {
            doc_lines.push(t);
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    // Reverse to get lines in original order and strip `//` prefix
    doc_lines.reverse();
    let text = doc_lines
        .iter()
        .map(|l| l.trim_start_matches("//").trim_start())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() { None } else { Some(text) }
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
            // control flow — not extracted as symbols
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
