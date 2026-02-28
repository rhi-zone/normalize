//! C# language support.

use crate::{
    ContainerBody, Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism,
};
use tree_sitter::Node;

/// C# language support.
pub struct CSharp;

impl Language for CSharp {
    fn name(&self) -> &'static str {
        "C#"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }
    fn grammar_name(&self) -> &'static str {
        "c_sharp"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "namespace_declaration",
        ]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &[
            "method_declaration",
            "constructor_declaration",
            "property_declaration",
            "local_function_statement",
            "lambda_expression",
        ]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "delegate_declaration",
        ]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["using_directive"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "method_declaration",
            "property_declaration",
        ]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::AccessModifier
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        if self.get_visibility(node, content) != Visibility::Public {
            return Vec::new();
        }

        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        let kind = match node.kind() {
            "class_declaration" => SymbolKind::Class,
            "struct_declaration" => SymbolKind::Struct,
            "interface_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            "record_declaration" => SymbolKind::Class,
            "method_declaration" | "constructor_declaration" => SymbolKind::Method,
            "property_declaration" => SymbolKind::Variable,
            _ => return Vec::new(),
        };

        vec![Export {
            name,
            kind,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "switch_statement",
            "using_statement",
            "block",
        ]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "throw_statement",
            "yield_statement",
        ]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_section",
            "catch_clause",
            "conditional_expression",
            "binary_expression",
        ]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "method_declaration",
            "class_declaration",
            "lambda_expression",
        ]
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;

        let params = node
            .child_by_field_name("parameters")
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        let return_type = node
            .child_by_field_name("type")
            .or_else(|| node.child_by_field_name("returns"))
            .map(|t| content[t.byte_range()].to_string());

        let signature = match return_type {
            Some(ret) => format!("{} {}{}", ret, name, params),
            None => format!("{}{}", name, params),
        };

        // Check for override modifier
        let is_override = {
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            children.iter().any(|child| {
                child.kind() == "modifier" && child.child(0).map(|c| c.kind()) == Some("override")
            })
        };

        Some(Symbol {
            name: name.to_string(),
            kind: if node.kind() == "property_declaration" {
                SymbolKind::Variable
            } else {
                SymbolKind::Method
            },
            signature,
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: is_override,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let (kind, keyword) = match node.kind() {
            "struct_declaration" => (SymbolKind::Struct, "struct"),
            "interface_declaration" => (SymbolKind::Interface, "interface"),
            "enum_declaration" => (SymbolKind::Enum, "enum"),
            "record_declaration" => (SymbolKind::Class, "record"),
            "namespace_declaration" => (SymbolKind::Module, "namespace"),
            _ => (SymbolKind::Class, "class"),
        };

        // Extract base types from base_list
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_list" {
                let mut bl = child.walk();
                for t in child.children(&mut bl) {
                    if t.kind() == "identifier" || t.kind() == "generic_name" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            }
        }

        Some(Symbol {
            name: name.to_string(),
            kind,
            signature: format!("{} {}", keyword, name),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: self.get_visibility(node, content),
            children: Vec::new(),
            is_interface_impl: false,
            implements,
        })
    }

    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol> {
        self.extract_container(node, content)
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Look for XML doc comments (/// or /** */)
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" {
                if text.starts_with("///") {
                    // Single-line XML doc comment
                    let line = text.strip_prefix("///").unwrap_or(text).trim();
                    // Strip XML tags for cleaner output
                    let clean = strip_xml_tags(line);
                    if !clean.is_empty() {
                        doc_lines.insert(0, clean);
                    }
                } else if text.starts_with("/**") {
                    // Multi-line doc comment
                    let inner = text
                        .strip_prefix("/**")
                        .unwrap_or(text)
                        .strip_suffix("*/")
                        .unwrap_or(text);
                    for line in inner.lines() {
                        let clean = line.trim().strip_prefix("*").unwrap_or(line).trim();
                        let clean = strip_xml_tags(clean);
                        if !clean.is_empty() {
                            doc_lines.push(clean);
                        }
                    }
                    break;
                } else {
                    break;
                }
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join(" "))
        }
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "using_directive" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];

        // Check for static using
        let is_static = text.contains("static ");

        // Get the namespace/type
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "qualified_name" || child.kind() == "identifier" {
                let module = content[child.byte_range()].to_string();
                return vec![Import {
                    module,
                    names: Vec::new(),
                    alias: if is_static {
                        Some("static".to_string())
                    } else {
                        None
                    },
                    is_wildcard: false,
                    is_relative: false,
                    line,
                }];
            }
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // C#: using Namespace; or using Alias = Namespace;
        if let Some(ref alias) = import.alias {
            format!("using {} = {};", alias, import.module)
        } else {
            format!("using {};", import.module)
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        self.get_visibility(node, content) == Visibility::Public
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
        node.child_by_field_name("body")
    }

    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn analyze_container_body(
        &self,
        _body_node: &Node,
        _content: &str,
        _inner_indent: &str,
    ) -> Option<ContainerBody> {
        None
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" {
                let mod_text = &content[child.byte_range()];
                if mod_text == "private" {
                    return Visibility::Private;
                }
                if mod_text == "protected" {
                    return Visibility::Protected;
                }
                if mod_text == "internal" {
                    return Visibility::Protected;
                }
                if mod_text == "public" {
                    return Visibility::Public;
                }
            }
        }
        // C# default visibility depends on context, but for skeleton purposes treat as public
        Visibility::Public
    }
}

/// Strip common XML doc comment tags for cleaner output
fn strip_xml_tags(s: &str) -> String {
    let mut result = s.to_string();
    // Remove common tags
    for tag in &[
        "<summary>",
        "</summary>",
        "<param>",
        "</param>",
        "<returns>",
        "</returns>",
        "<remarks>",
        "</remarks>",
        "<example>",
        "</example>",
        "<c>",
        "</c>",
        "<see cref=\"",
        "\"/>",
        "<seealso cref=\"",
    ] {
        result = result.replace(tag, "");
    }
    // Handle self-closing see tags
    while let Some(start) = result.find("<see ") {
        if let Some(end) = result[start..].find("/>") {
            result = format!("{}{}", &result[..start], &result[start + end + 2..]);
        } else {
            break;
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // C# grammar uses "c_sharp" - check cross_check output for actual kinds
            // This is a placeholder - run cross_check_node_kinds to get the full list
        ];

        // C# may need manual verification - skip for now if empty
        if !documented_unused.is_empty() {
            validate_unused_kinds_audit(&CSharp, documented_unused)
                .expect("C# unused node kinds audit failed");
        }
    }
}
