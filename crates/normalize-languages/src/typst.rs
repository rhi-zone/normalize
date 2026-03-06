//! Typst language support.

use crate::{Import, Language};
use tree_sitter::Node;

/// Typst language support.
pub struct Typst;

impl Language for Typst {
    fn name(&self) -> &'static str {
        "Typst"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["typ"]
    }
    fn grammar_name(&self) -> &'static str {
        "typst"
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: text.contains('*'),
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Typst: #import "file.typ" or #import "file.typ": item1, item2
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("#import \"{}\"", import.module)
        } else if import.is_wildcard {
            format!("#import \"{}\": *", import.module)
        } else {
            format!("#import \"{}\": {}", import.module, names_to_use.join(", "))
        }
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
            // Math mode
            "formula",
            // Control flow (not function definitions)
            "return",
            // Inline lambdas are not top-level definitions
            "lambda",
            // Loop constructs — not definition kinds
            "for", "while",
            // Module system — not a symbol definition
            "import",
            // Block expression — container body, not a top-level definition
            "block",
        ];
        validate_unused_kinds_audit(&Typst, documented_unused)
            .expect("Typst unused node kinds audit failed");
    }
}
