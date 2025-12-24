//! Code anchors: named locations in source code.
//!
//! Anchors are stable references to code locations (functions, classes, variables)
//! that can be used for structural edits instead of line numbers.

use moss_core::{tree_sitter, Parsers};
use moss_languages::{support_for_path, Language, SymbolKind as LangSymbolKind};
use std::path::Path;

/// Type of code anchor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorType {
    Function,
    Class,
    Method,
    Variable,
    Import,
    Struct,
    Enum,
    Trait,
}

impl AnchorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnchorType::Function => "function",
            AnchorType::Class => "class",
            AnchorType::Method => "method",
            AnchorType::Variable => "variable",
            AnchorType::Import => "import",
            AnchorType::Struct => "struct",
            AnchorType::Enum => "enum",
            AnchorType::Trait => "trait",
        }
    }
}

fn convert_symbol_kind(kind: LangSymbolKind, in_container: bool) -> AnchorType {
    match kind {
        LangSymbolKind::Function => if in_container { AnchorType::Method } else { AnchorType::Function },
        LangSymbolKind::Method => AnchorType::Method,
        LangSymbolKind::Class => AnchorType::Class,
        LangSymbolKind::Struct => AnchorType::Struct,
        LangSymbolKind::Enum => AnchorType::Enum,
        LangSymbolKind::Interface | LangSymbolKind::Trait => AnchorType::Trait,
        LangSymbolKind::Variable | LangSymbolKind::Constant => AnchorType::Variable,
        LangSymbolKind::Module | LangSymbolKind::Type | LangSymbolKind::Heading => AnchorType::Class,
    }
}

/// A code anchor (named code location)
#[derive(Debug, Clone)]
pub struct Anchor {
    pub name: String,
    pub anchor_type: AnchorType,
    pub context: Option<String>, // Parent class/struct/impl
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}

impl Anchor {
    /// Format as a reference string
    pub fn reference(&self) -> String {
        if let Some(ctx) = &self.context {
            format!("{}.{}", ctx, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Result of anchor extraction
#[allow(dead_code)] // file_path provides context for the result
pub struct AnchorsResult {
    pub anchors: Vec<Anchor>,
    pub file_path: String,
}

pub struct AnchorExtractor {
    parsers: Parsers,
}

impl AnchorExtractor {
    pub fn new() -> Self {
        Self {
            parsers: Parsers::new(),
        }
    }

    pub fn extract(&self, path: &Path, content: &str) -> AnchorsResult {
        let support = match support_for_path(path) {
            Some(s) => s,
            None => return AnchorsResult {
                anchors: Vec::new(),
                file_path: path.to_string_lossy().to_string(),
            },
        };

        let anchors = self.extract_with_trait(content, support);

        AnchorsResult {
            anchors,
            file_path: path.to_string_lossy().to_string(),
        }
    }

    fn extract_with_trait(&self, content: &str, support: &dyn Language) -> Vec<Anchor> {
        let tree = match self.parsers.parse_with_grammar(support.grammar_name(), content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let root = tree.root_node();
        let mut anchors = Vec::new();
        self.collect_with_trait(&mut root.walk(), content, support, &mut anchors, None);
        anchors
    }

    fn collect_with_trait(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        support: &dyn Language,
        anchors: &mut Vec<Anchor>,
        context: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            // Check for container (class, impl, etc.)
            if support.container_kinds().contains(&kind) {
                if let Some(sym) = support.extract_container(&node, content) {
                    let sym_name = sym.name.clone();
                    anchors.push(Anchor {
                        name: sym.name,
                        anchor_type: convert_symbol_kind(sym.kind, context.is_some()),
                        context: context.map(String::from),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        signature: Some(sym.signature),
                    });

                    // Recurse into container to find methods
                    if cursor.goto_first_child() {
                        self.collect_with_trait(cursor, content, support, anchors, Some(&sym_name));
                        cursor.goto_parent();
                    }
                    if cursor.goto_next_sibling() {
                        continue;
                    }
                    break;
                }
            }

            // Check for function
            if support.function_kinds().contains(&kind) {
                if let Some(sym) = support.extract_function(&node, content, context.is_some()) {
                    anchors.push(Anchor {
                        name: sym.name,
                        anchor_type: convert_symbol_kind(sym.kind, context.is_some()),
                        context: context.map(String::from),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        signature: Some(sym.signature),
                    });
                }
            }

            // Check for type (struct, enum, trait - when not a container)
            if support.type_kinds().contains(&kind) && !support.container_kinds().contains(&kind) {
                if let Some(sym) = support.extract_type(&node, content) {
                    anchors.push(Anchor {
                        name: sym.name,
                        anchor_type: convert_symbol_kind(sym.kind, context.is_some()),
                        context: context.map(String::from),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        signature: Some(sym.signature),
                    });
                }
            }

            // Check for imports
            if support.import_kinds().contains(&kind) {
                for imp in support.extract_imports(&node, content) {
                    // Add each imported name as an anchor
                    if imp.names.is_empty() {
                        // Module-level import
                        let name = imp.alias.unwrap_or_else(|| {
                            imp.module.split(&['/', '.', ':']).last().unwrap_or(&imp.module).to_string()
                        });
                        anchors.push(Anchor {
                            name,
                            anchor_type: AnchorType::Import,
                            context: None,
                            start_line: imp.line,
                            end_line: imp.line,
                            signature: None,
                        });
                    } else {
                        for name in imp.names {
                            anchors.push(Anchor {
                                name,
                                anchor_type: AnchorType::Import,
                                context: None,
                                start_line: imp.line,
                                end_line: imp.line,
                                signature: None,
                            });
                        }
                    }
                }
            }

            // Recurse into children (but not for containers, handled above)
            if !support.container_kinds().contains(&kind) && cursor.goto_first_child() {
                self.collect_with_trait(cursor, content, support, anchors, context);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    // Legacy methods - kept for reference, now using trait-based extraction
    #[allow(dead_code)]
    fn extract_python(&self, content: &str) -> Vec<Anchor> {
        let tree = match self.parsers.parse_with_grammar("python", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut anchors = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_python_anchors(&mut cursor, content, &mut anchors, None);
        anchors
    }

    #[allow(dead_code)]
    fn collect_python_anchors(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        anchors: &mut Vec<Anchor>,
        context: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_definition" | "async_function_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();

                        // Get signature
                        let params = node.child_by_field_name("parameters");
                        let params_text = params
                            .map(|p| content[p.byte_range()].to_string())
                            .unwrap_or_else(|| "()".to_string());

                        let is_async = kind == "async_function_definition";
                        let prefix = if is_async { "async def" } else { "def" };
                        let signature = format!("{} {}{}", prefix, name, params_text);

                        anchors.push(Anchor {
                            name,
                            anchor_type: if context.is_some() {
                                AnchorType::Method
                            } else {
                                AnchorType::Function
                            },
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(signature),
                        });
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();

                        anchors.push(Anchor {
                            name: name.clone(),
                            anchor_type: AnchorType::Class,
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(format!("class {}", name)),
                        });

                        // Recurse into class body
                        if cursor.goto_first_child() {
                            self.collect_python_anchors(cursor, content, anchors, Some(&name));
                            cursor.goto_parent();
                        }
                        if cursor.goto_next_sibling() {
                            continue;
                        }
                        break;
                    }
                }
                "assignment" => {
                    // Top-level variable assignments
                    if context.is_none() {
                        if let Some(left) = node.child_by_field_name("left") {
                            if left.kind() == "identifier" {
                                let name = content[left.byte_range()].to_string();
                                // Only include UPPER_CASE constants
                                if name.chars().all(|c| c.is_uppercase() || c == '_') {
                                    anchors.push(Anchor {
                                        name,
                                        anchor_type: AnchorType::Variable,
                                        context: None,
                                        start_line: node.start_position().row + 1,
                                        end_line: node.end_position().row + 1,
                                        signature: None,
                                    });
                                }
                            }
                        }
                    }
                }
                "import_statement" | "import_from_statement" => {
                    // Extract imported names
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                                let name = if child.kind() == "aliased_import" {
                                    child
                                        .child_by_field_name("alias")
                                        .map(|n| content[n.byte_range()].to_string())
                                        .or_else(|| {
                                            child
                                                .child_by_field_name("name")
                                                .map(|n| content[n.byte_range()].to_string())
                                        })
                                } else {
                                    Some(content[child.byte_range()].to_string())
                                };

                                if let Some(name) = name {
                                    anchors.push(Anchor {
                                        name,
                                        anchor_type: AnchorType::Import,
                                        context: None,
                                        start_line: node.start_position().row + 1,
                                        end_line: node.end_position().row + 1,
                                        signature: None,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            // Recurse into children
            if kind != "class_definition" && cursor.goto_first_child() {
                self.collect_python_anchors(cursor, content, anchors, context);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[allow(dead_code)]
    fn extract_rust(&self, content: &str) -> Vec<Anchor> {
        let tree = match self.parsers.parse_with_grammar("rust", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut anchors = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_rust_anchors(&mut cursor, content, &mut anchors, None);
        anchors
    }

    #[allow(dead_code)]
    fn collect_rust_anchors(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        anchors: &mut Vec<Anchor>,
        context: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();

                        // Get params
                        let params = node.child_by_field_name("parameters");
                        let params_text = params
                            .map(|p| content[p.byte_range()].to_string())
                            .unwrap_or_else(|| "()".to_string());

                        let signature = format!("fn {}{}", name, params_text);

                        anchors.push(Anchor {
                            name,
                            anchor_type: if context.is_some() {
                                AnchorType::Method
                            } else {
                                AnchorType::Function
                            },
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(signature),
                        });
                    }
                }
                "struct_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        anchors.push(Anchor {
                            name: name.clone(),
                            anchor_type: AnchorType::Struct,
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(format!("struct {}", name)),
                        });
                    }
                }
                "enum_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        anchors.push(Anchor {
                            name: name.clone(),
                            anchor_type: AnchorType::Enum,
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(format!("enum {}", name)),
                        });
                    }
                }
                "trait_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        anchors.push(Anchor {
                            name: name.clone(),
                            anchor_type: AnchorType::Trait,
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: Some(format!("trait {}", name)),
                        });
                    }
                }
                "impl_item" => {
                    // Get the type being implemented
                    if let Some(type_node) = node.child_by_field_name("type") {
                        let type_name = content[type_node.byte_range()].to_string();

                        // Recurse into impl body
                        if let Some(body) = node.child_by_field_name("body") {
                            let mut body_cursor = body.walk();
                            if body_cursor.goto_first_child() {
                                self.collect_rust_anchors(
                                    &mut body_cursor,
                                    content,
                                    anchors,
                                    Some(&type_name),
                                );
                            }
                        }
                    }
                    if cursor.goto_next_sibling() {
                        continue;
                    }
                    break;
                }
                "const_item" | "static_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        anchors.push(Anchor {
                            name,
                            anchor_type: AnchorType::Variable,
                            context: context.map(String::from),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            signature: None,
                        });
                    }
                }
                "use_declaration" => {
                    // Extract used names (simplified - just gets the last segment)
                    let text = &content[node.byte_range()];
                    if let Some(name) = text.split("::").last() {
                        let name = name
                            .trim_end_matches(';')
                            .trim()
                            .trim_start_matches('{')
                            .trim_end_matches('}');
                        if !name.contains(',') && !name.is_empty() {
                            anchors.push(Anchor {
                                name: name.to_string(),
                                anchor_type: AnchorType::Import,
                                context: None,
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                                signature: None,
                            });
                        }
                    }
                }
                _ => {}
            }

            // Recurse into children
            if kind != "impl_item" && cursor.goto_first_child() {
                self.collect_rust_anchors(cursor, content, anchors, context);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Find a specific anchor by name (fuzzy match)
    pub fn find_anchor(&self, path: &Path, content: &str, query: &str) -> Vec<Anchor> {
        let result = self.extract(path, content);
        let query_lower = query.to_lowercase();

        result
            .anchors
            .into_iter()
            .filter(|a| {
                let name_lower = a.name.to_lowercase();
                let ref_lower = a.reference().to_lowercase();

                // Exact match
                name_lower == query_lower
                    || ref_lower == query_lower
                    // Prefix match
                    || name_lower.starts_with(&query_lower)
                    // Contains
                    || name_lower.contains(&query_lower)
                    || ref_lower.contains(&query_lower)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_python_anchors() {
        let extractor = AnchorExtractor::new();
        let content = r#"
import os

def foo(x: int) -> str:
    return str(x)

class Bar:
    def method(self):
        pass
"#;
        let result = extractor.extract(&PathBuf::from("test.py"), content);

        let names: Vec<_> = result.anchors.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"os"));
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"Bar"));
        assert!(names.contains(&"method"));

        // Check method has context
        let method = result.anchors.iter().find(|a| a.name == "method").unwrap();
        assert_eq!(method.context.as_deref(), Some("Bar"));
        assert_eq!(method.anchor_type, AnchorType::Method);
    }

    #[test]
    fn test_rust_anchors() {
        let extractor = AnchorExtractor::new();
        let content = r#"
use std::path::Path;

struct Foo {
    x: i32,
}

impl Foo {
    fn new(x: i32) -> Self {
        Self { x }
    }
}
"#;
        let result = extractor.extract(&PathBuf::from("test.rs"), content);

        let names: Vec<_> = result.anchors.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"new"));

        // Check method has context
        let method = result.anchors.iter().find(|a| a.name == "new").unwrap();
        assert_eq!(method.context.as_deref(), Some("Foo"));
    }

    #[test]
    fn test_find_anchor() {
        let extractor = AnchorExtractor::new();
        let content = r#"
def hello_world():
    pass

def hello_there():
    pass
"#;
        let matches = extractor.find_anchor(&PathBuf::from("test.py"), content, "hello");
        assert_eq!(matches.len(), 2);
    }
}
