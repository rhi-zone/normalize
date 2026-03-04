//! jq language support.

use crate::{Import, Language, Symbol, Visibility, simple_function_symbol};
use tree_sitter::Node;

/// jq language support.
pub struct Jq;

impl Language for Jq {
    fn name(&self) -> &'static str {
        "jq"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jq"]
    }
    fn grammar_name(&self) -> &'static str {
        "jq"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(simple_function_symbol(node, content, name, None))
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // import "path" as name;
        if let Some(rest) = text.strip_prefix("import ") {
            let module = rest.split('"').nth(1).map(|s| s.to_string());
            let alias = rest
                .split(" as ")
                .nth(1)
                .and_then(|s| s.split(';').next())
                .map(|s| s.trim().to_string());

            if let Some(module) = module {
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias,
                    is_wildcard: false,
                    is_relative: true,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // jq: import "module" as name
        format!("import \"{}\"", import.module)
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
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
            "catch", "elif", "else", "format", "identifier", "import_", "moduleheader",
            "programbody",
        ];
        validate_unused_kinds_audit(&Jq, documented_unused)
            .expect("jq unused node kinds audit failed");
    }
}
