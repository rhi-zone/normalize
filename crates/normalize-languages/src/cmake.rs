//! CMake language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
    simple_function_symbol,
};
use tree_sitter::Node;

/// CMake language support.
pub struct CMake;

impl Language for CMake {
    fn name(&self) -> &'static str {
        "CMake"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cmake"]
    }
    fn grammar_name(&self) -> &'static str {
        "cmake"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["function_def", "macro_def"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_def", "macro_def"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["normal_command"] // include(), find_package()
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_def", "macro_def"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NotApplicable
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "function_def" | "macro_def" => SymbolKind::Function,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["function_def", "macro_def"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["if_condition", "foreach_loop", "while_loop"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_condition",
            "elseif_command",
            "foreach_loop",
            "while_loop",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["function_def", "macro_def", "if_condition", "foreach_loop"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(
            node,
            content,
            name,
            self.extract_docstring(node, content),
        ))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_function(node, content, false)
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // CMake uses # for comments
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "line_comment" {
                let line = text.strip_prefix('#').unwrap_or(text).trim();
                doc_lines.push(line.to_string());
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "normal_command" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // include(file), find_package(pkg)
        if text.starts_with("include(") || text.starts_with("find_package(") {
            let inner = text
                .split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
                .map(|s| s.trim().to_string());

            if let Some(module) = inner {
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: text.starts_with("include("),
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // CMake: include(file) or find_package(pkg)
        format!("include({})", import.module)
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, _symbol: &crate::Symbol) -> bool {
        false
    }

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // CMake function_def/macro_def: body is an unnamed child of kind "body"
        let mut c = node.walk();
        node.children(&mut c).find(|&child| child.kind() == "body")
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
        // body node: "\n  message(...)\n  set(...)" — raw statements after opening newline
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // function(name args...) - name is first argument
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "argument" {
                return Some(&content[child.byte_range()]);
            }
        }
        None
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
            "block", "block_command", "block_def", "body", "else", "else_command",
            "elseif", "endblock", "endblock_command", "endforeach", "endforeach_command",
            "endfunction", "endfunction_command", "endif", "endif_command", "endwhile",
            "endwhile_command", "foreach", "foreach_command", "function",
            "function_command", "identifier", "if", "if_command", "while",
            "while_command",
        ];
        validate_unused_kinds_audit(&CMake, documented_unused)
            .expect("CMake unused node kinds audit failed");
    }
}
