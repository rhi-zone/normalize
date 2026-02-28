//! Haskell language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
    simple_function_symbol,
};
use tree_sitter::Node;

/// Haskell language support.
pub struct Haskell;

impl Language for Haskell {
    fn name(&self) -> &'static str {
        "Haskell"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["hs", "lhs"]
    }
    fn grammar_name(&self) -> &'static str {
        "haskell"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["data_type", "newtype", "type_synomym", "class", "instance"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function", "signature"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &["data_type", "newtype", "type_synomym"]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["import"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function", "data_type", "newtype", "class"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport // module export list
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "function" | "signature" => SymbolKind::Function,
            "data_type" | "newtype" => SymbolKind::Struct,
            "type_synomym" => SymbolKind::Type,
            "class" => SymbolKind::Interface,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["let", "where", "do", "lambda"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["conditional", "case", "match", "guard"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["conditional", "case", "match", "guard", "lambda"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["function", "let", "where", "do", "case"]
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
        let name = self.node_name(node, content)?;

        let (kind, keyword) = match node.kind() {
            "data_type" => (SymbolKind::Struct, "data"),
            "newtype" => (SymbolKind::Struct, "newtype"),
            "type_synomym" => (SymbolKind::Type, "type"),
            "class" => (SymbolKind::Interface, "class"),
            "instance" => {
                // instance MyClass Foo where ...
                // name = "MyClass" (typeclass), type_patterns contains "Foo" (implementing type)
                let implements = vec![name.to_string()];
                // Try to get the implementing type from type_patterns for a better signature
                let mut type_name = None;
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32)
                        && child.kind() == "type_patterns"
                    {
                        let mut cursor = child.walk();
                        for tp_child in child.children(&mut cursor) {
                            if tp_child.kind() == "name" {
                                type_name = Some(content[tp_child.byte_range()].to_string());
                                break;
                            }
                        }
                    }
                }
                let sig = if let Some(ref tn) = type_name {
                    format!("instance {} {}", name, tn)
                } else {
                    format!("instance {}", name)
                };
                return Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    signature: sig,
                    docstring: self.extract_docstring(node, content),
                    attributes: Vec::new(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: Visibility::Public,
                    children: Vec::new(),
                    is_interface_impl: true,
                    implements,
                });
            }
            _ => return None,
        };

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Haskell uses -- | or {- | -} for Haddock docs
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" {
                if text.starts_with("-- |") || text.starts_with("-- ^") {
                    let line = text
                        .strip_prefix("-- |")
                        .or_else(|| text.strip_prefix("-- ^"))
                        .unwrap_or(text)
                        .trim();
                    doc_lines.push(line.to_string());
                } else if text.starts_with("--") {
                    let line = text.strip_prefix("--").unwrap_or(text).trim();
                    doc_lines.push(line.to_string());
                }
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
        if node.kind() != "import" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // Extract module name after "import" keyword
        // import qualified Data.Map as M
        let parts: Vec<&str> = text.split_whitespace().collect();
        let mut idx = 1;
        if parts.get(idx) == Some(&"qualified") {
            idx += 1;
        }

        if let Some(module) = parts.get(idx) {
            return vec![Import {
                module: module.to_string(),
                names: Vec::new(),
                alias: None,
                is_wildcard: !text.contains('('),
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Haskell: import Module or import Module (a, b, c)
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import {}", import.module)
        } else {
            format!("import {} ({})", import.module, names_to_use.join(", "))
        }
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        // tree-sitter-haskell uses "declarations" (not "where") for the body
        node.child_by_field_name("declarations")
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
        // class_declarations / instance_declarations contain declarations
        // directly, with no enclosing keywords in the node itself
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
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
            "associated_type", "class_declarations", "constructor",
            "constructor_operator", "constructor_synonym", "constructor_synonyms",
            "data_constructor", "data_constructors", "declarations",
            "default_types", "do_module", "explicit_type", "export", "exports",
            "forall", "forall_required", "foreign_export", "foreign_import",
            "function_head_parens", "gadt_constructor", "gadt_constructors",
            "generator", "import_list", "import_name", "import_package", "imports",
            "instance_declarations", "lambda_case", "lambda_cases",
            "linear_function", "list_comprehension", "modifier", "module",
            "module_export", "module_id", "multi_way_if", "newtype_constructor",
            "operator", "qualified", "qualifiers", "quantified_variables",
            "quasiquote_body", "quoted_expression", "quoted_type", "transform",
            "type_application", "type_binder", "type_family",
            "type_family_injectivity", "type_family_result", "type_instance",
            "type_params", "type_patterns", "type_role",
            "typed_quote",
        ];
        validate_unused_kinds_audit(&Haskell, documented_unused)
            .expect("Haskell unused node kinds audit failed");
    }
}
