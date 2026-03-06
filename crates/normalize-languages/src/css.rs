//! CSS language support (parse only, minimal skeleton).

use crate::Language;
use tree_sitter::Node;

/// CSS language support.
pub struct Css;

impl Language for Css {
    fn name(&self) -> &'static str {
        "CSS"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["css", "scss"]
    }
    fn grammar_name(&self) -> &'static str {
        "css"
    }

    fn has_symbols(&self) -> bool {
        false
    }

    // CSS has no functions/containers/types in the traditional sense

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
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
            "binary_expression", "block", "call_expression", "charset_statement",
            "class_name", "class_selector", "declaration", "function_name",
            "identifier", "import_statement", "important", "important_value",
            "keyframe_block", "keyframe_block_list", "keyframes_statement",
            "media_statement", "namespace_statement", "postcss_statement",
            "pseudo_class_selector", "scope_statement", "supports_statement",
        ];
        validate_unused_kinds_audit(&Css, documented_unused)
            .expect("CSS unused node kinds audit failed");
    }
}
