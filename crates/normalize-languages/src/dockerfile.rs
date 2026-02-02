//! Dockerfile language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Dockerfile language support.
pub struct Dockerfile;

impl Language for Dockerfile {
    fn name(&self) -> &'static str {
        "Dockerfile"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dockerfile"]
    }
    fn grammar_name(&self) -> &'static str {
        "dockerfile"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    // Dockerfiles have stages (FROM ... AS name) that act as containers
    fn container_kinds(&self) -> &'static [&'static str] {
        &["from_instruction"]
    }

    // No functions in Dockerfile
    fn function_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["from_instruction"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["from_instruction"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NotApplicable
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if node.kind() != "from_instruction" {
            return Vec::new();
        }

        // Extract the stage name (FROM image AS name)
        if let Some(name) = self.extract_stage_name(node, content) {
            return vec![Export {
                name,
                kind: SymbolKind::Module,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[]
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(
        &self,
        _node: &Node,
        _content: &str,
        _in_container: bool,
    ) -> Option<Symbol> {
        None
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() != "from_instruction" {
            return None;
        }

        // Extract base image
        let image_name = self.extract_image_name(node, content)?;
        let stage_name = self.extract_stage_name(node, content);

        let name = stage_name.clone().unwrap_or_else(|| image_name.clone());
        let signature = if let Some(stage) = stage_name {
            format!("FROM {} AS {}", image_name, stage)
        } else {
            format!("FROM {}", image_name)
        };

        Some(Symbol {
            name,
            kind: SymbolKind::Module,
            signature,
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "from_instruction" {
            return Vec::new();
        }

        if let Some(image) = self.extract_image_name(node, content) {
            return vec![Import {
                module: image,
                names: Vec::new(),
                alias: self.extract_stage_name(node, content),
                is_wildcard: false,
                is_relative: false,
                line: node.start_position().row + 1,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Dockerfile: FROM image
        format!("FROM {}", import.module)
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

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }
    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

impl Dockerfile {
    /// Extract the image name from a FROM instruction
    fn extract_image_name(&self, node: &Node, content: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "image_spec" {
                return Some(content[child.byte_range()].to_string());
            }
        }
        None
    }

    /// Extract the stage name from a FROM instruction (FROM image AS name)
    fn extract_stage_name(&self, node: &Node, content: &str) -> Option<String> {
        let mut cursor = node.walk();
        let mut found_as = false;
        for child in node.children(&mut cursor) {
            if found_as && child.kind() == "image_alias" {
                return Some(content[child.byte_range()].to_string());
            }
            if child.kind() == "as_instruction" {
                found_as = true;
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
            // All Dockerfile instruction types (we don't track these as symbols)
            "add_instruction", "arg_instruction", "cmd_instruction", "copy_instruction",
            "cross_build_instruction", "entrypoint_instruction", "env_instruction",
            "expose_instruction", "healthcheck_instruction", "heredoc_block",
            "label_instruction", "maintainer_instruction", "onbuild_instruction",
            "run_instruction", "shell_instruction", "stopsignal_instruction",
            "user_instruction", "volume_instruction", "workdir_instruction",
        ];

        validate_unused_kinds_audit(&Dockerfile, documented_unused)
            .expect("Dockerfile unused node kinds audit failed");
    }
}
