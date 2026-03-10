//! Device Tree source file support.

use crate::{ContainerBody, Import, Language, LanguageSymbols};
use tree_sitter::Node;

/// Device Tree language support.
pub struct DeviceTree;

impl Language for DeviceTree {
    fn name(&self) -> &'static str {
        "DeviceTree"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dts", "dtsi"]
    }
    fn grammar_name(&self) -> &'static str {
        "devicetree"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "preproc_include" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let module = text
            .split('"')
            .nth(1)
            .or_else(|| text.split('<').nth(1).and_then(|s| s.split('>').next()))
            .map(|s| s.to_string());

        if let Some(module) = module {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: text.contains('"'),
                line: node.start_position().row + 1,
            }];
        }
        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Device Tree: /include/ "file.dtsi"
        format!("/include/ \"{}\"", import.module)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // DeviceTree node spans "identifier { ... };" — use node itself for brace analysis
        Some(*node)
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        // node: "identifier { properties... }" — brace-delimited body
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }
}

impl LanguageSymbols for DeviceTree {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Preprocessor
            "preproc_if", "preproc_ifdef", "preproc_else", "preproc_elif",
            "preproc_elifdef", "preproc_function_def",
            // Expressions
            "unary_expression", "binary_expression", "conditional_expression",
            "parenthesized_expression", "call_expression",
            // Other
            "identifier", "omit_if_no_ref",
        ];
        validate_unused_kinds_audit(&DeviceTree, documented_unused)
            .expect("DeviceTree unused node kinds audit failed");
    }
}
