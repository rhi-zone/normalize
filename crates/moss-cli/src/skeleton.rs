//! AST-based code skeleton extraction.
//!
//! Extracts function/class signatures with optional docstrings.

use std::path::Path;
use tree_sitter::Parser;

/// A code symbol with its signature
#[derive(Debug, Clone)]
pub struct SkeletonSymbol {
    pub name: String,
    pub kind: &'static str, // "class", "function", "method"
    pub signature: String,
    pub docstring: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub children: Vec<SkeletonSymbol>,
}

/// Result of skeleton extraction
pub struct SkeletonResult {
    pub symbols: Vec<SkeletonSymbol>,
    pub file_path: String,
}

impl SkeletonResult {
    /// Format skeleton as text output
    pub fn format(&self, include_docstrings: bool) -> String {
        let mut lines = Vec::new();
        format_symbols(&self.symbols, include_docstrings, 0, &mut lines);
        lines.join("\n")
    }
}

fn format_symbols(
    symbols: &[SkeletonSymbol],
    include_docstrings: bool,
    indent: usize,
    lines: &mut Vec<String>,
) {
    let prefix = "    ".repeat(indent);

    for sym in symbols {
        lines.push(format!("{}{}:", prefix, sym.signature));

        if include_docstrings {
            if let Some(doc) = &sym.docstring {
                // First line only for brevity
                let first_line = doc.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    lines.push(format!("{}    \"\"\"{}\"\"\"", prefix, first_line));
                }
            }
        }

        if sym.children.is_empty() {
            lines.push(format!("{}    ...", prefix));
        } else {
            format_symbols(&sym.children, include_docstrings, indent + 1, lines);
        }

        lines.push(String::new()); // Blank line between symbols
    }
}

pub struct SkeletonExtractor {
    python_parser: Parser,
    rust_parser: Parser,
}

impl SkeletonExtractor {
    pub fn new() -> Self {
        let mut python_parser = Parser::new();
        python_parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("Failed to load Python grammar");

        let mut rust_parser = Parser::new();
        rust_parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        Self {
            python_parser,
            rust_parser,
        }
    }

    pub fn extract(&mut self, path: &Path, content: &str) -> SkeletonResult {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let symbols = match ext {
            "py" => self.extract_python(content),
            "rs" => self.extract_rust(content),
            _ => Vec::new(),
        };

        SkeletonResult {
            symbols,
            file_path: path.to_string_lossy().to_string(),
        }
    }

    fn extract_python(&mut self, content: &str) -> Vec<SkeletonSymbol> {
        let tree = match self.python_parser.parse(content, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut symbols = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_python_symbols(&mut cursor, content, &mut symbols, false);
        symbols
    }

    fn collect_python_symbols(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        symbols: &mut Vec<SkeletonSymbol>,
        in_class: bool,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_definition" | "async_function_definition" => {
                    if let Some(sym) = self.extract_python_function(&node, content, in_class) {
                        symbols.push(sym);
                    }
                }
                "class_definition" => {
                    if let Some(sym) = self.extract_python_class(&node, content) {
                        symbols.push(sym);
                    }
                    // Skip children - we handle them in extract_python_class
                    if cursor.goto_next_sibling() {
                        continue;
                    }
                    break;
                }
                _ => {}
            }

            // Recurse into children (except for class definitions)
            if kind != "class_definition" && cursor.goto_first_child() {
                self.collect_python_symbols(cursor, content, symbols, in_class);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn extract_python_function(
        &self,
        node: &tree_sitter::Node,
        content: &str,
        in_class: bool,
    ) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Skip private methods unless they're dunder methods
        if name.starts_with('_') && !name.starts_with("__") {
            return None;
        }

        let is_async = node.kind() == "async_function_definition";
        let prefix = if is_async { "async def" } else { "def" };

        // Extract parameters
        let params = node.child_by_field_name("parameters");
        let params_text = params
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        // Extract return type
        let return_type = node.child_by_field_name("return_type");
        let return_text = return_type
            .map(|r| format!(" -> {}", &content[r.byte_range()]))
            .unwrap_or_default();

        let signature = format!("{} {}{}{}", prefix, name, params_text, return_text);

        // Extract docstring
        let docstring = self.extract_python_docstring(node, content);

        Some(SkeletonSymbol {
            name,
            kind: if in_class { "method" } else { "function" },
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children: Vec::new(),
        })
    }

    fn extract_python_class(
        &self,
        node: &tree_sitter::Node,
        content: &str,
    ) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Skip private classes
        if name.starts_with('_') && !name.starts_with("__") {
            return None;
        }

        // Extract base classes
        let mut bases = Vec::new();
        if let Some(args_node) = node.child_by_field_name("superclasses") {
            let args_text = &content[args_node.byte_range()];
            // Remove parentheses
            let trimmed = args_text.trim_start_matches('(').trim_end_matches(')');
            if !trimmed.is_empty() {
                bases.push(trimmed.to_string());
            }
        }

        let signature = if bases.is_empty() {
            format!("class {}", name)
        } else {
            format!("class {}({})", name, bases.join(", "))
        };

        // Extract docstring
        let docstring = self.extract_python_docstring(node, content);

        // Extract methods
        let mut children = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            if cursor.goto_first_child() {
                self.collect_python_symbols(&mut cursor, content, &mut children, true);
            }
        }

        Some(SkeletonSymbol {
            name,
            kind: "class",
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children,
        })
    }

    fn extract_python_docstring(&self, node: &tree_sitter::Node, content: &str) -> Option<String> {
        // Look for docstring in body
        let body = node.child_by_field_name("body")?;
        let first_child = body.child(0)?;

        if first_child.kind() == "expression_statement" {
            let expr = first_child.child(0)?;
            if expr.kind() == "string" {
                let text = &content[expr.byte_range()];
                // Remove quotes and strip
                let doc = text
                    .trim_start_matches("\"\"\"")
                    .trim_start_matches("'''")
                    .trim_start_matches('"')
                    .trim_start_matches('\'')
                    .trim_end_matches("\"\"\"")
                    .trim_end_matches("'''")
                    .trim_end_matches('"')
                    .trim_end_matches('\'')
                    .trim();
                if !doc.is_empty() {
                    return Some(doc.to_string());
                }
            }
        }
        None
    }

    fn extract_rust(&mut self, content: &str) -> Vec<SkeletonSymbol> {
        let tree = match self.rust_parser.parse(content, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut symbols = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_rust_symbols(&mut cursor, content, &mut symbols, None);
        symbols
    }

    fn collect_rust_symbols(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        symbols: &mut Vec<SkeletonSymbol>,
        impl_name: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_item" => {
                    if let Some(sym) = self.extract_rust_function(&node, content, impl_name) {
                        symbols.push(sym);
                    }
                }
                "struct_item" => {
                    if let Some(sym) = self.extract_rust_struct(&node, content) {
                        symbols.push(sym);
                    }
                }
                "enum_item" => {
                    if let Some(sym) = self.extract_rust_enum(&node, content) {
                        symbols.push(sym);
                    }
                }
                "trait_item" => {
                    if let Some(sym) = self.extract_rust_trait(&node, content) {
                        symbols.push(sym);
                    }
                }
                "impl_item" => {
                    // Get the type being implemented
                    if let Some(type_node) = node.child_by_field_name("type") {
                        let type_name = &content[type_node.byte_range()];

                        // Find impl body and recurse
                        if let Some(body) = node.child_by_field_name("body") {
                            let mut body_cursor = body.walk();
                            if body_cursor.goto_first_child() {
                                let mut methods = Vec::new();
                                self.collect_rust_symbols(
                                    &mut body_cursor,
                                    content,
                                    &mut methods,
                                    Some(type_name),
                                );

                                // Add methods to existing struct symbol or create impl symbol
                                if !methods.is_empty() {
                                    // Find existing struct/enum and add methods
                                    let found = symbols.iter_mut().find(|s| s.name == type_name);
                                    if let Some(existing) = found {
                                        existing.children.extend(methods);
                                    } else {
                                        // Create impl symbol
                                        symbols.push(SkeletonSymbol {
                                            name: type_name.to_string(),
                                            kind: "impl",
                                            signature: format!("impl {}", type_name),
                                            docstring: None,
                                            start_line: node.start_position().row + 1,
                                            end_line: node.end_position().row + 1,
                                            children: methods,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if cursor.goto_next_sibling() {
                        continue;
                    }
                    break;
                }
                _ => {}
            }

            // Recurse into children (except for impl blocks)
            if kind != "impl_item" && cursor.goto_first_child() {
                self.collect_rust_symbols(cursor, content, symbols, impl_name);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn extract_rust_function(
        &self,
        node: &tree_sitter::Node,
        content: &str,
        impl_name: Option<&str>,
    ) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Get visibility
        let mut vis = String::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "visibility_modifier" {
                    vis = format!("{} ", &content[child.byte_range()]);
                    break;
                }
            }
        }

        // Get parameters
        let params = node.child_by_field_name("parameters");
        let params_text = params
            .map(|p| content[p.byte_range()].to_string())
            .unwrap_or_else(|| "()".to_string());

        // Get return type
        let return_type = node.child_by_field_name("return_type");
        let return_text = return_type
            .map(|r| format!(" -> {}", &content[r.byte_range()]))
            .unwrap_or_default();

        let signature = format!("{}fn {}{}{}", vis, name, params_text, return_text);

        // Extract doc comment (look for preceding line_comment or block_comment)
        let docstring = self.extract_rust_doc_comment(node, content);

        Some(SkeletonSymbol {
            name,
            kind: if impl_name.is_some() { "method" } else { "function" },
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children: Vec::new(),
        })
    }

    fn extract_rust_struct(&self, node: &tree_sitter::Node, content: &str) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Get visibility
        let mut vis = String::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "visibility_modifier" {
                    vis = format!("{} ", &content[child.byte_range()]);
                    break;
                }
            }
        }

        let signature = format!("{}struct {}", vis, name);
        let docstring = self.extract_rust_doc_comment(node, content);

        Some(SkeletonSymbol {
            name,
            kind: "struct",
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children: Vec::new(),
        })
    }

    fn extract_rust_enum(&self, node: &tree_sitter::Node, content: &str) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Get visibility
        let mut vis = String::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "visibility_modifier" {
                    vis = format!("{} ", &content[child.byte_range()]);
                    break;
                }
            }
        }

        let signature = format!("{}enum {}", vis, name);
        let docstring = self.extract_rust_doc_comment(node, content);

        Some(SkeletonSymbol {
            name,
            kind: "enum",
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children: Vec::new(),
        })
    }

    fn extract_rust_trait(&self, node: &tree_sitter::Node, content: &str) -> Option<SkeletonSymbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = content[name_node.byte_range()].to_string();

        // Get visibility
        let mut vis = String::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "visibility_modifier" {
                    vis = format!("{} ", &content[child.byte_range()]);
                    break;
                }
            }
        }

        let signature = format!("{}trait {}", vis, name);
        let docstring = self.extract_rust_doc_comment(node, content);

        // Extract trait methods
        let mut children = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            if cursor.goto_first_child() {
                self.collect_rust_symbols(&mut cursor, content, &mut children, Some(&name));
            }
        }

        Some(SkeletonSymbol {
            name,
            kind: "trait",
            signature,
            docstring,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            children,
        })
    }

    fn extract_rust_doc_comment(&self, node: &tree_sitter::Node, content: &str) -> Option<String> {
        // Look for doc comments before the node
        let lines: Vec<&str> = content.lines().collect();
        let start_line = node.start_position().row;

        if start_line == 0 {
            return None;
        }

        // Check preceding lines for doc comments
        let mut doc_lines = Vec::new();
        for i in (0..start_line).rev() {
            let line = lines.get(i)?.trim();
            if line.starts_with("///") {
                let doc = line.trim_start_matches("///").trim();
                doc_lines.insert(0, doc.to_string());
            } else if line.starts_with("//!") {
                // Module-level doc, skip
                break;
            } else if line.is_empty() {
                // Empty line, stop if we have content
                if !doc_lines.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_python_skeleton() {
        let mut extractor = SkeletonExtractor::new();
        let content = r#"
def foo(x: int) -> str:
    """Convert int to string."""
    return str(x)

class Bar:
    """A bar class."""

    def method(self, y: float) -> bool:
        """Check something."""
        return y > 0
"#;
        let result = extractor.extract(&PathBuf::from("test.py"), content);
        assert_eq!(result.symbols.len(), 2);

        let foo = &result.symbols[0];
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.kind, "function");
        assert!(foo.signature.contains("def foo(x: int) -> str"));
        assert_eq!(foo.docstring.as_deref(), Some("Convert int to string."));

        let bar = &result.symbols[1];
        assert_eq!(bar.name, "Bar");
        assert_eq!(bar.kind, "class");
        assert_eq!(bar.children.len(), 1);
        assert_eq!(bar.children[0].name, "method");
    }

    #[test]
    fn test_rust_skeleton() {
        let mut extractor = SkeletonExtractor::new();
        let content = r#"
/// A simple struct
pub struct Foo {
    x: i32,
}

impl Foo {
    /// Create a new Foo
    pub fn new(x: i32) -> Self {
        Self { x }
    }
}
"#;
        let result = extractor.extract(&PathBuf::from("test.rs"), content);

        // Should have struct with method from impl
        let foo = result.symbols.iter().find(|s| s.name == "Foo").unwrap();
        assert_eq!(foo.kind, "struct");
        assert!(foo.signature.contains("pub struct Foo"));
        assert_eq!(foo.children.len(), 1);
        assert_eq!(foo.children[0].name, "new");
    }

    #[test]
    fn test_format_skeleton() {
        let mut extractor = SkeletonExtractor::new();
        let content = r#"
def hello(name: str) -> str:
    """Say hello."""
    return f"Hello, {name}"
"#;
        let result = extractor.extract(&PathBuf::from("test.py"), content);
        let formatted = result.format(true);

        assert!(formatted.contains("def hello(name: str) -> str:"));
        assert!(formatted.contains("\"\"\"Say hello.\"\"\""));
    }
}
