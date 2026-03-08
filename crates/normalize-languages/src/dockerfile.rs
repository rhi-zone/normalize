//! Dockerfile language support.

use crate::{Import, Language};
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

    // Dockerfiles have stages (FROM ... AS name) that act as containers

    // No functions in Dockerfile

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
            // structural node, not extracted as symbols
            "from_instruction",
        ];

        validate_unused_kinds_audit(&Dockerfile, documented_unused)
            .expect("Dockerfile unused node kinds audit failed");
    }
}
