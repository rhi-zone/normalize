//! DOT/Graphviz language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// DOT (Graphviz) language support.
pub struct Dot;

impl Language for Dot {
    fn name(&self) -> &'static str {
        "DOT"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dot", "gv"]
    }
    fn grammar_name(&self) -> &'static str {
        "dot"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["graph", "digraph", "subgraph"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["graph", "digraph", "subgraph"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AllPublic
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        match node.kind() {
            "graph" | "digraph" | "subgraph" => {
                let name = self
                    .node_name(node, content)
                    .unwrap_or("unnamed")
                    .to_string();
                vec![Export {
                    name,
                    kind: SymbolKind::Module,
                    line: node.start_position().row + 1,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["graph", "digraph", "subgraph"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[]
    }
    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["subgraph"]
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
        let _kind_name = match node.kind() {
            "graph" => "graph",
            "digraph" => "digraph",
            "subgraph" => "subgraph",
            _ => return None,
        };

        let name = self
            .node_name(node, content)
            .unwrap_or("unnamed")
            .to_string();
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Module,
            signature: first_line.trim().to_string(),
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
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, _import: &Import, _names: Option<&[&str]>) -> String {
        // Graphviz DOT has no imports
        String::new()
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
        node.child_by_field_name("body")
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
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .or_else(|| node.child_by_field_name("id"))
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
            "block",      // Statement block (body)
            "identifier", // Node/edge identifiers
            "operator",   // Edge operators (-> or --)
        ];
        validate_unused_kinds_audit(&Dot, documented_unused)
            .expect("DOT unused node kinds audit failed");
    }
}
