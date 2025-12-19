use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor};

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Import,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Method => "method",
            SymbolKind::Variable => "variable",
            SymbolKind::Import => "import",
        }
    }
}

pub struct SymbolParser {
    python_parser: Parser,
    rust_parser: Parser,
}

impl SymbolParser {
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

    pub fn parse_file(&mut self, path: &Path, content: &str) -> Vec<Symbol> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "py" => self.parse_python(content),
            "rs" => self.parse_rust(content),
            _ => Vec::new(),
        }
    }

    fn parse_python(&mut self, content: &str) -> Vec<Symbol> {
        let tree = match self.python_parser.parse(content, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut symbols = Vec::new();
        let root = tree.root_node();

        // Walk the tree to find functions, classes, methods
        let mut cursor = root.walk();
        self.collect_python_symbols(&mut cursor, content, &mut symbols, None);

        symbols
    }

    fn collect_python_symbols(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        symbols: &mut Vec<Symbol>,
        parent: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_definition" | "async_function_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = &content[name_node.byte_range()];
                        let symbol_kind = if parent.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: symbol_kind,
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            parent: parent.map(String::from),
                        });
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = &content[name_node.byte_range()];
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Class,
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            parent: parent.map(String::from),
                        });

                        // Recurse into class body to find methods
                        if cursor.goto_first_child() {
                            self.collect_python_symbols(cursor, content, symbols, Some(name));
                            cursor.goto_parent();
                        }
                        if cursor.goto_next_sibling() {
                            continue;
                        }
                        break;
                    }
                }
                _ => {}
            }

            // Recurse into children (but not for class definitions, handled above)
            if kind != "class_definition" && cursor.goto_first_child() {
                self.collect_python_symbols(cursor, content, symbols, parent);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn parse_rust(&mut self, content: &str) -> Vec<Symbol> {
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
        symbols: &mut Vec<Symbol>,
        parent: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "function_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = &content[name_node.byte_range()];
                        let symbol_kind = if parent.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: symbol_kind,
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            parent: parent.map(String::from),
                        });
                    }
                }
                "struct_item" | "enum_item" | "trait_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = &content[name_node.byte_range()];
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Class, // Use Class for struct/enum/trait
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            parent: parent.map(String::from),
                        });
                    }
                }
                "impl_item" => {
                    // Find the type being implemented
                    let impl_name = node
                        .child_by_field_name("type")
                        .map(|n| content[n.byte_range()].to_string());

                    if let Some(name) = &impl_name {
                        // Recurse into impl block to find methods
                        if cursor.goto_first_child() {
                            self.collect_rust_symbols(cursor, content, symbols, Some(name));
                            cursor.goto_parent();
                        }
                        if cursor.goto_next_sibling() {
                            continue;
                        }
                        break;
                    }
                }
                _ => {}
            }

            // Recurse into children (but not for impl blocks, handled above)
            if kind != "impl_item" && cursor.goto_first_child() {
                self.collect_rust_symbols(cursor, content, symbols, parent);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Find a symbol by name in a file
    pub fn find_symbol(&mut self, path: &Path, content: &str, name: &str) -> Option<Symbol> {
        let symbols = self.parse_file(path, content);
        symbols.into_iter().find(|s| s.name == name)
    }

    /// Extract the source code for a symbol
    pub fn extract_symbol_source(&mut self, path: &Path, content: &str, name: &str) -> Option<String> {
        let symbol = self.find_symbol(path, content, name)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = symbol.start_line.saturating_sub(1);
        let end = symbol.end_line.min(lines.len());
        Some(lines[start..end].join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_python_function() {
        let mut parser = SymbolParser::new();
        let content = r#"
def foo():
    pass

def bar(x):
    return x
"#;
        let symbols = parser.parse_file(&PathBuf::from("test.py"), content);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "foo");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[1].name, "bar");
    }

    #[test]
    fn test_parse_python_class() {
        let mut parser = SymbolParser::new();
        let content = r#"
class Foo:
    def method(self):
        pass
"#;
        let symbols = parser.parse_file(&PathBuf::from("test.py"), content);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Foo");
        assert_eq!(symbols[0].kind, SymbolKind::Class);
        assert_eq!(symbols[1].name, "method");
        assert_eq!(symbols[1].kind, SymbolKind::Method);
        assert_eq!(symbols[1].parent, Some("Foo".to_string()));
    }

    #[test]
    fn test_parse_rust_function() {
        let mut parser = SymbolParser::new();
        let content = r#"
fn foo() {}

fn bar(x: i32) -> i32 {
    x
}
"#;
        let symbols = parser.parse_file(&PathBuf::from("test.rs"), content);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "foo");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_symbol_source() {
        let mut parser = SymbolParser::new();
        let content = r#"def foo():
    return 42

def bar():
    pass"#;
        let source = parser.extract_symbol_source(&PathBuf::from("test.py"), content, "foo");
        assert!(source.is_some());
        assert!(source.unwrap().contains("return 42"));
    }
}
