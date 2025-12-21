use std::path::Path;
use tree_sitter::Parser;

/// Result of finding a symbol in a file
#[derive(Debug)]
pub struct SymbolLocation {
    pub name: String,
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub indent: String,
}

/// Editor for structural code modifications
pub struct Editor {
    python_parser: Parser,
    rust_parser: Parser,
}

impl Editor {
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

    /// Find a symbol by name in a file
    pub fn find_symbol(&mut self, path: &Path, content: &str, name: &str) -> Option<SymbolLocation> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let tree = match ext {
            "py" => self.python_parser.parse(content, None)?,
            "rs" => self.rust_parser.parse(content, None)?,
            _ => return None,
        };

        let root = tree.root_node();
        self.find_symbol_in_node(root, content, name, ext)
    }

    fn find_symbol_in_node(
        &self,
        node: tree_sitter::Node,
        content: &str,
        name: &str,
        ext: &str,
    ) -> Option<SymbolLocation> {
        // Check if this node is the symbol we're looking for
        if let Some(loc) = self.check_node_is_symbol(&node, content, name, ext) {
            return Some(loc);
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(loc) = self.find_symbol_in_node(child, content, name, ext) {
                return Some(loc);
            }
        }

        None
    }

    fn check_node_is_symbol(
        &self,
        node: &tree_sitter::Node,
        content: &str,
        name: &str,
        ext: &str,
    ) -> Option<SymbolLocation> {
        let kind = node.kind();
        let symbol_kind = match ext {
            "py" => match kind {
                "function_definition" | "async_function_definition" => Some("function"),
                "class_definition" => Some("class"),
                _ => None,
            },
            "rs" => match kind {
                "function_item" => Some("function"),
                "struct_item" | "enum_item" | "trait_item" => Some("class"),
                "impl_item" => Some("impl"),
                _ => None,
            },
            _ => None,
        }?;

        // Get the name of this symbol
        let name_node = node.child_by_field_name("name")?;
        let symbol_name = &content[name_node.byte_range()];

        if symbol_name != name {
            return None;
        }

        // Calculate indentation from the start of the line
        let start_byte = node.start_byte();
        let line_start = content[..start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let indent = &content[line_start..start_byte];
        let indent = indent.chars().take_while(|c| c.is_whitespace()).collect::<String>();

        Some(SymbolLocation {
            name: symbol_name.to_string(),
            kind: symbol_kind.to_string(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            indent,
        })
    }

    /// Delete a symbol from the content
    pub fn delete_symbol(&mut self, content: &str, loc: &SymbolLocation) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol
        let line_start = content[..loc.start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);

        // Find the end of the line containing the symbol end (include trailing newline)
        let mut end_byte = loc.end_byte;
        if end_byte < content.len() && content.as_bytes()[end_byte] == b'\n' {
            end_byte += 1;
        }

        result.push_str(&content[..line_start]);
        result.push_str(&content[end_byte..]);

        result
    }

    /// Replace a symbol with new content
    pub fn replace_symbol(&mut self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..loc.start_byte]);
        result.push_str(&indented);
        result.push_str(&content[loc.end_byte..]);

        result
    }

    /// Insert content before a symbol
    pub fn insert_before(&mut self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol
        let line_start = content[..loc.start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..line_start]);
        result.push_str(&indented);
        result.push('\n');
        result.push_str(&content[line_start..]);

        result
    }

    /// Insert content after a symbol
    pub fn insert_after(&mut self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        // Find the end of the line after the symbol
        let end_pos = if loc.end_byte < content.len() && content.as_bytes()[loc.end_byte] == b'\n' {
            loc.end_byte + 1
        } else {
            loc.end_byte
        };

        result.push_str(&content[..end_pos]);
        if !content[..end_pos].ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&indented);
        result.push('\n');
        result.push_str(&content[end_pos..]);

        result
    }

    /// Insert content at the beginning of a file
    pub fn prepend_to_file(&self, content: &str, new_content: &str) -> String {
        let mut result = String::new();
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(content);
        result
    }

    /// Insert content at the end of a file
    pub fn append_to_file(&self, content: &str, new_content: &str) -> String {
        let mut result = String::new();
        result.push_str(content);
        if !content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    /// Apply indentation to content
    fn apply_indent(&self, content: &str, indent: &str) -> String {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    format!("{}{}", indent, line)
                } else if line.is_empty() {
                    line.to_string()
                } else {
                    format!("{}{}", indent, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_find_python_function() {
        let mut editor = Editor::new();
        let content = r#"
def foo():
    pass

def bar():
    return 42
"#;
        let loc = editor.find_symbol(&PathBuf::from("test.py"), content, "bar");
        assert!(loc.is_some());
        let loc = loc.unwrap();
        assert_eq!(loc.name, "bar");
        assert_eq!(loc.kind, "function");
    }

    #[test]
    fn test_delete_symbol() {
        let mut editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor.find_symbol(&PathBuf::from("test.py"), content, "bar").unwrap();
        let result = editor.delete_symbol(content, &loc);
        assert!(!result.contains("bar"));
        assert!(result.contains("foo"));
    }

    #[test]
    fn test_insert_before() {
        let mut editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor.find_symbol(&PathBuf::from("test.py"), content, "bar").unwrap();
        let result = editor.insert_before(content, &loc, "def baz():\n    pass");
        assert!(result.contains("baz"));
        assert!(result.find("baz").unwrap() < result.find("bar").unwrap());
    }
}
