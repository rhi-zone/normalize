use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::{Language, support_for_path};
use normalize_view::skeleton::{SkeletonExtractor, SkeletonSymbol};
use std::path::Path;

pub use normalize_languages::ContainerBody;

/// Result of finding a symbol in a file
#[derive(Debug)]
#[allow(dead_code)] // Fields used by Debug trait and for edit operations
pub struct SymbolLocation {
    pub name: String,
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub indent: String,
}

/// Convert a 1-based line number to byte offset in content.
/// Clamps to content length for safety (last line may not have trailing newline).
pub fn line_to_byte(content: &str, line: usize) -> usize {
    let pos: usize = content
        .lines()
        .take(line.saturating_sub(1))
        .map(|l| l.len() + 1)
        .sum();
    pos.min(content.len())
}

/// Editor for structural code modifications
pub struct Editor {}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {}
    }

    /// Find a symbol by name in a file (uses skeleton extractor)
    pub fn find_symbol(
        &self,
        path: &Path,
        content: &str,
        name: &str,
        case_insensitive: bool,
    ) -> Option<SymbolLocation> {
        let extractor = SkeletonExtractor::new();
        let result = extractor.extract(path, content);

        fn search_symbols(
            symbols: &[SkeletonSymbol],
            name: &str,
            content: &str,
            case_insensitive: bool,
        ) -> Option<SymbolLocation> {
            for sym in symbols {
                let matches = if case_insensitive {
                    sym.name.eq_ignore_ascii_case(name)
                } else {
                    sym.name == name
                };
                if matches {
                    let start_byte = line_to_byte(content, sym.start_line);
                    let end_byte = line_to_byte(content, sym.end_line + 1);

                    return Some(SymbolLocation {
                        name: sym.name.clone(),
                        kind: sym.kind.as_str().to_string(),
                        start_byte,
                        end_byte,
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        indent: String::new(),
                    });
                }
                // Search children
                if let Some(loc) = search_symbols(&sym.children, name, content, case_insensitive) {
                    return Some(loc);
                }
            }
            None
        }

        search_symbols(&result.symbols, name, content, case_insensitive)
    }

    /// Delete a symbol from the content
    pub fn delete_symbol(&self, content: &str, loc: &SymbolLocation) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol
        let line_start = content[..loc.start_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Find the end of the line containing the symbol end (include trailing newline)
        let mut end_byte = loc.end_byte;
        if end_byte < content.len() && content.as_bytes()[end_byte] == b'\n' {
            end_byte += 1;
        }

        // Smart whitespace: consume trailing blank lines to avoid double-blanks
        // But only if there's already a blank line before the symbol
        let has_blank_before =
            line_start >= 2 && &content[line_start.saturating_sub(2)..line_start] == "\n\n";

        if has_blank_before {
            // Consume trailing blank lines (up to one full blank line)
            while end_byte < content.len() && content.as_bytes()[end_byte] == b'\n' {
                end_byte += 1;
                // Only consume one blank line worth
                if end_byte < content.len() && content.as_bytes()[end_byte] != b'\n' {
                    break;
                }
            }
        }

        result.push_str(&content[..line_start]);
        result.push_str(&content[end_byte..]);

        result
    }

    /// Replace a symbol with new content
    pub fn replace_symbol(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..loc.start_byte]);
        result.push_str(&indented);
        result.push_str(&content[loc.end_byte..]);

        result
    }

    /// Count blank lines before a position
    fn count_blank_lines_before(&self, content: &str, pos: usize) -> usize {
        let mut count = 0usize;
        let mut i = pos;
        while i > 0 {
            i -= 1;
            if content.as_bytes()[i] == b'\n' {
                count += 1;
            } else if !content.as_bytes()[i].is_ascii_whitespace() {
                break;
            }
        }
        count.saturating_sub(1) // Don't count the newline ending the previous line
    }

    /// Count blank lines after a position (after any trailing newline)
    fn count_blank_lines_after(&self, content: &str, pos: usize) -> usize {
        let mut count = 0;
        let mut i = pos;
        // Skip past the first newline (end of current symbol)
        if i < content.len() && content.as_bytes()[i] == b'\n' {
            i += 1;
        }
        while i < content.len() {
            if content.as_bytes()[i] == b'\n' {
                count += 1;
                i += 1;
            } else if content.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Insert content before a symbol
    pub fn insert_before(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol
        let line_start = content[..loc.start_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Detect spacing convention: how many blank lines before this symbol?
        let blank_lines = self.count_blank_lines_before(content, line_start);
        // +1 for the newline ending the content, +N for N blank lines
        let spacing = "\n".repeat(blank_lines.max(1) + 1);

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..line_start]);
        result.push_str(&indented);
        result.push_str(&spacing);
        result.push_str(&content[line_start..]);

        result
    }

    /// Insert content after a symbol
    pub fn insert_after(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        // Find the end of the symbol (include trailing newline)
        let end_pos = if loc.end_byte < content.len() && content.as_bytes()[loc.end_byte] == b'\n' {
            loc.end_byte + 1
        } else {
            loc.end_byte
        };

        // Detect spacing convention: how many blank lines after this symbol?
        let blank_lines = self.count_blank_lines_after(content, loc.end_byte);
        // end_pos already includes trailing newline, so just add N newlines for N blank lines
        let spacing = "\n".repeat(blank_lines.max(1));

        // Find where the next non-blank content starts
        let mut next_content_pos = end_pos;
        while next_content_pos < content.len() && content.as_bytes()[next_content_pos] == b'\n' {
            next_content_pos += 1;
        }

        result.push_str(&content[..end_pos]);
        result.push_str(&spacing);
        result.push_str(&indented);

        if next_content_pos < content.len() {
            // +1 for the newline ending the inserted content
            result.push_str(&"\n".repeat(blank_lines.max(1) + 1));
            result.push_str(&content[next_content_pos..]);
        } else {
            result.push('\n');
        }

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

    /// Find the body of a container symbol (class, impl block, markdown section) for prepend/append
    pub fn find_container_body(
        &self,
        path: &Path,
        content: &str,
        name: &str,
    ) -> Option<ContainerBody> {
        let support = support_for_path(path)?;
        let grammar = support.grammar_name();
        let tree = parse_with_grammar(grammar, content)?;
        let root = tree.root_node();
        find_container_body_in_node(root, content, name, support)
    }

    /// Prepend content inside a container (class/impl body)
    pub fn prepend_to_container(
        &self,
        content: &str,
        body: &ContainerBody,
        new_content: &str,
    ) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &body.inner_indent);

        result.push_str(&content[..body.content_start]);

        // Add the new content
        result.push_str(&indented);
        result.push('\n');

        // Add spacing if there's existing content
        if !body.is_empty {
            result.push('\n');
        }

        result.push_str(&content[body.content_start..]);

        result
    }

    /// Append content inside a container (class/impl body)
    pub fn append_to_container(
        &self,
        content: &str,
        body: &ContainerBody,
        new_content: &str,
    ) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &body.inner_indent);

        // Trim trailing whitespace/newlines from existing content
        let mut end_pos = body.content_end;
        while end_pos > 0
            && content
                .as_bytes()
                .get(end_pos - 1)
                .map(|&b| b == b'\n' || b == b' ')
                == Some(true)
        {
            end_pos -= 1;
        }

        result.push_str(&content[..end_pos]);

        // Add blank line before new content (Python/Rust convention for methods)
        if !body.is_empty {
            result.push_str("\n\n");
        } else {
            result.push('\n');
        }

        // Add the new content
        result.push_str(&indented);
        result.push('\n');

        result.push_str(&content[body.content_end..]);

        result
    }

    /// Apply indentation to content
    pub fn apply_indent(&self, content: &str, indent: &str) -> String {
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

fn find_container_body_in_node(
    node: tree_sitter::Node,
    content: &str,
    name: &str,
    support: &dyn Language,
) -> Option<ContainerBody> {
    let kind = node.kind();

    if support.container_kinds().contains(&kind)
        && let Some(container_name) = support.node_name(&node, content)
        && container_name == name
        && let Some(body_node) = support.container_body(&node)
    {
        let start_byte = node.start_byte();
        let line_start = content[..start_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let container_indent: String = content[line_start..start_byte]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let inner_indent = format!("{}    ", container_indent);

        return support.analyze_container_body(&body_node, content, &inner_indent);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(body) = find_container_body_in_node(child, content, name, support) {
            return Some(body);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_find_python_function() {
        let editor = Editor::new();
        let content = r#"
def foo():
    pass

def bar():
    return 42
"#;
        let loc = editor.find_symbol(&PathBuf::from("test.py"), content, "bar", false);
        assert!(loc.is_some());
        let loc = loc.unwrap();
        assert_eq!(loc.name, "bar");
        assert_eq!(loc.kind, "function");
    }

    #[test]
    fn test_delete_symbol() {
        let editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor
            .find_symbol(&PathBuf::from("test.py"), content, "bar", false)
            .unwrap();
        let result = editor.delete_symbol(content, &loc);
        assert!(!result.contains("bar"));
        assert!(result.contains("foo"));
    }

    #[test]
    fn test_insert_before() {
        let editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor
            .find_symbol(&PathBuf::from("test.py"), content, "bar", false)
            .unwrap();
        let result = editor.insert_before(content, &loc, "def baz():\n    pass");
        assert!(result.contains("baz"));
        assert!(result.find("baz").unwrap() < result.find("bar").unwrap());
    }

    #[test]
    fn test_prepend_to_python_class() {
        let editor = Editor::new();
        let content = r#"class Foo:
    """Docstring."""

    def first(self):
        pass
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.py"), content, "Foo")
            .unwrap();
        let result =
            editor.prepend_to_container(content, &body, "def new_method(self):\n    return 1");
        // New method should appear after docstring but before first
        assert!(result.contains("new_method"));
        let docstring_pos = result.find("Docstring").unwrap();
        let new_method_pos = result.find("new_method").unwrap();
        let first_pos = result.find("first").unwrap();
        assert!(docstring_pos < new_method_pos);
        assert!(new_method_pos < first_pos);
    }

    #[test]
    fn test_append_to_python_class() {
        let editor = Editor::new();
        let content = r#"class Foo:
    def first(self):
        pass

    def second(self):
        return 42
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.py"), content, "Foo")
            .unwrap();
        let result = editor.append_to_container(content, &body, "def last(self):\n    return 99");
        // New method should appear after second
        assert!(result.contains("last"));
        let second_pos = result.find("second").unwrap();
        let last_pos = result.find("last").unwrap();
        assert!(second_pos < last_pos);
    }

    #[test]
    fn test_prepend_to_rust_impl() {
        let editor = Editor::new();
        let content = r#"impl Foo {
    fn first(&self) -> i32 {
        1
    }
}
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.rs"), content, "Foo")
            .unwrap();
        let result =
            editor.prepend_to_container(content, &body, "fn new() -> Self {\n    Self {}\n}");
        assert!(result.contains("new"));
        let new_pos = result.find("new").unwrap();
        let first_pos = result.find("first").unwrap();
        assert!(new_pos < first_pos);
    }

    #[test]
    fn test_append_to_rust_impl() {
        let editor = Editor::new();
        let content = r#"impl Foo {
    fn first(&self) -> i32 {
        1
    }
}
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.rs"), content, "Foo")
            .unwrap();
        let result =
            editor.append_to_container(content, &body, "fn last(&self) -> i32 {\n    99\n}");
        assert!(result.contains("last"));
        let first_pos = result.find("first").unwrap();
        let last_pos = result.find("last").unwrap();
        assert!(first_pos < last_pos);
        // Should still have closing brace
        assert!(result.contains("}"));
    }
}
