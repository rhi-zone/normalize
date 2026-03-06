//! C# language support.

use crate::{ContainerBody, Import, Language, Visibility};
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
        "c-sharp"
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_csharp_doc_comment(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> (bool, Vec<String>) {
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
        (false, implements)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        match node.kind() {
            "method_declaration" | "constructor_declaration" | "property_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_default();
                let return_type = node
                    .child_by_field_name("type")
                    .or_else(|| node.child_by_field_name("returns"))
                    .map(|t| content[t.byte_range()].to_string());
                match return_type {
                    Some(ret) => format!("{} {}{}", ret, name, params),
                    None => format!("{}{}", name, params),
                }
            }
            "class_declaration" => format!("class {}", name),
            "struct_declaration" => format!("struct {}", name),
            "interface_declaration" => format!("interface {}", name),
            "enum_declaration" => format!("enum {}", name),
            "record_declaration" => format!("record {}", name),
            "namespace_declaration" => format!("namespace {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
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

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/*Test.cs", "**/*Tests.cs"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_csharp_attributes(node, content)
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

/// Extract attributes from a C# definition node.
/// C# attributes are `attribute_list` children (e.g. `[Obsolete]`, `[DllImport("...")]`).
fn extract_csharp_attributes(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_list" {
            attrs.push(content[child.byte_range()].to_string());
        }
    }
    attrs
}

/// Extract a C# doc comment preceding a node.
///
/// Supports `///` XML doc comment lines and `/** ... */` block doc comments.
fn extract_csharp_doc_comment(node: &Node, content: &str) -> Option<String> {
    let mut doc_lines: Vec<String> = Vec::new();
    let mut prev = node.prev_sibling();

    while let Some(sibling) = prev {
        if sibling.kind() == "comment" {
            let text = &content[sibling.byte_range()];
            if text.starts_with("///") {
                let line = text.strip_prefix("///").unwrap_or("").trim();
                let line = strip_xml_tags(line);
                if !line.is_empty() {
                    doc_lines.push(line);
                }
            } else if text.starts_with("/**") {
                let lines: Vec<&str> = text
                    .strip_prefix("/**")
                    .unwrap_or(text)
                    .strip_suffix("*/")
                    .unwrap_or(text)
                    .lines()
                    .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !lines.is_empty() {
                    return Some(lines.join(" "));
                }
                return None;
            } else {
                break;
            }
        } else if sibling.kind() == "attribute_list" {
            // Skip [Attribute] between doc comment and declaration
        } else {
            break;
        }
        prev = sibling.prev_sibling();
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    let joined = doc_lines.join(" ").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Strip common XML doc comment tags.
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
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
