//! Module dependency extraction.
//!
//! Extracts imports and exports from source files.

use std::path::Path;
use tree_sitter::Parser;

/// An import statement
#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    pub names: Vec<String>, // Names imported (empty for "import x")
    pub alias: Option<String>,
    pub line: usize,
    pub is_relative: bool,
}

/// An exported symbol
#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: &'static str, // "function", "class", "variable"
    pub line: usize,
}

/// Dependency information for a file
pub struct DepsResult {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub file_path: String,
}

impl DepsResult {
    /// Format as compact text
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        if !self.imports.is_empty() {
            lines.push("# Imports".to_string());
            for imp in &self.imports {
                let prefix = if imp.is_relative {
                    format!(".{}", imp.module)
                } else {
                    imp.module.clone()
                };

                if imp.names.is_empty() {
                    let alias = imp.alias.as_ref().map(|a| format!(" as {}", a)).unwrap_or_default();
                    lines.push(format!("import {}{}", prefix, alias));
                } else {
                    lines.push(format!("from {} import {}", prefix, imp.names.join(", ")));
                }
            }
            lines.push(String::new());
        }

        if !self.exports.is_empty() {
            lines.push("# Exports".to_string());
            for exp in &self.exports {
                if exp.kind != "variable" {
                    lines.push(format!("{}: {}", exp.kind, exp.name));
                }
            }
        }

        lines.join("\n").trim_end().to_string()
    }
}

pub struct DepsExtractor {
    python_parser: Parser,
    rust_parser: Parser,
}

impl DepsExtractor {
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

    pub fn extract(&mut self, path: &Path, content: &str) -> DepsResult {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let (imports, exports) = match ext {
            "py" => self.extract_python(content),
            "rs" => self.extract_rust(content),
            _ => (Vec::new(), Vec::new()),
        };

        DepsResult {
            imports,
            exports,
            file_path: path.to_string_lossy().to_string(),
        }
    }

    fn extract_python(&mut self, content: &str) -> (Vec<Import>, Vec<Export>) {
        let tree = match self.python_parser.parse(content, None) {
            Some(t) => t,
            None => return (Vec::new(), Vec::new()),
        };

        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_python_deps(&mut cursor, content, &mut imports, &mut exports, false);
        (imports, exports)
    }

    fn collect_python_deps(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        imports: &mut Vec<Import>,
        exports: &mut Vec<Export>,
        in_class: bool,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "import_statement" => {
                    // import x, import x as y
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "dotted_name" {
                                let module = content[child.byte_range()].to_string();
                                imports.push(Import {
                                    module,
                                    names: Vec::new(),
                                    alias: None,
                                    line: node.start_position().row + 1,
                                    is_relative: false,
                                });
                            } else if child.kind() == "aliased_import" {
                                let name_node = child.child_by_field_name("name");
                                let alias_node = child.child_by_field_name("alias");
                                if let Some(name) = name_node {
                                    let module = content[name.byte_range()].to_string();
                                    let alias = alias_node.map(|a| content[a.byte_range()].to_string());
                                    imports.push(Import {
                                        module,
                                        names: Vec::new(),
                                        alias,
                                        line: node.start_position().row + 1,
                                        is_relative: false,
                                    });
                                }
                            }
                        }
                    }
                }
                "import_from_statement" => {
                    // from x import y, z
                    let module_node = node.child_by_field_name("module_name");
                    let module = module_node
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();

                    // Check for relative import (starts with .)
                    let text = &content[node.byte_range()];
                    let is_relative = text.contains("from .");

                    let mut names = Vec::new();
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "import_name" || child.kind() == "dotted_name" {
                                // Skip the module name
                                if Some(child) != module_node {
                                    names.push(content[child.byte_range()].to_string());
                                }
                            } else if child.kind() == "aliased_import" {
                                if let Some(name) = child.child_by_field_name("name") {
                                    names.push(content[name.byte_range()].to_string());
                                }
                            }
                        }
                    }

                    imports.push(Import {
                        module,
                        names,
                        alias: None,
                        line: node.start_position().row + 1,
                        is_relative,
                    });
                }
                "function_definition" | "async_function_definition" => {
                    if !in_class {
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let name = content[name_node.byte_range()].to_string();
                            if !name.starts_with('_') {
                                exports.push(Export {
                                    name,
                                    kind: "function",
                                    line: node.start_position().row + 1,
                                });
                            }
                        }
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        if !name.starts_with('_') {
                            exports.push(Export {
                                name,
                                kind: "class",
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                    // Mark that we're inside a class
                    if cursor.goto_first_child() {
                        self.collect_python_deps(cursor, content, imports, exports, true);
                        cursor.goto_parent();
                    }
                    if cursor.goto_next_sibling() {
                        continue;
                    }
                    break;
                }
                _ => {}
            }

            // Recurse
            if kind != "class_definition" && cursor.goto_first_child() {
                self.collect_python_deps(cursor, content, imports, exports, in_class);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn extract_rust(&mut self, content: &str) -> (Vec<Import>, Vec<Export>) {
        let tree = match self.rust_parser.parse(content, None) {
            Some(t) => t,
            None => return (Vec::new(), Vec::new()),
        };

        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        self.collect_rust_deps(&mut cursor, content, &mut imports, &mut exports);
        (imports, exports)
    }

    fn collect_rust_deps(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        imports: &mut Vec<Import>,
        exports: &mut Vec<Export>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                "use_declaration" => {
                    let text = &content[node.byte_range()];
                    // Extract module path (simplified)
                    let module = text
                        .trim_start_matches("use ")
                        .trim_end_matches(';')
                        .trim();

                    // Extract names if it's a use with braces
                    let mut names = Vec::new();
                    if module.contains('{') {
                        if let Some(brace_start) = module.find('{') {
                            let prefix = &module[..brace_start].trim_end_matches("::");
                            if let Some(brace_end) = module.find('}') {
                                let items = &module[brace_start + 1..brace_end];
                                for item in items.split(',') {
                                    names.push(item.trim().to_string());
                                }
                            }
                            imports.push(Import {
                                module: prefix.to_string(),
                                names,
                                alias: None,
                                line: node.start_position().row + 1,
                                is_relative: prefix.starts_with("crate") || prefix.starts_with("self") || prefix.starts_with("super"),
                            });
                        }
                    } else {
                        imports.push(Import {
                            module: module.to_string(),
                            names: Vec::new(),
                            alias: None,
                            line: node.start_position().row + 1,
                            is_relative: module.starts_with("crate") || module.starts_with("self") || module.starts_with("super"),
                        });
                    }
                }
                "function_item" => {
                    // Check for pub
                    let mut is_pub = false;
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "visibility_modifier" {
                                is_pub = content[child.byte_range()].contains("pub");
                                break;
                            }
                        }
                    }
                    if is_pub {
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let name = content[name_node.byte_range()].to_string();
                            exports.push(Export {
                                name,
                                kind: "function",
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
                "struct_item" | "enum_item" | "trait_item" => {
                    let mut is_pub = false;
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "visibility_modifier" {
                                is_pub = content[child.byte_range()].contains("pub");
                                break;
                            }
                        }
                    }
                    if is_pub {
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let name = content[name_node.byte_range()].to_string();
                            let item_kind = match kind {
                                "struct_item" => "struct",
                                "enum_item" => "enum",
                                "trait_item" => "trait",
                                _ => "type",
                            };
                            exports.push(Export {
                                name,
                                kind: item_kind,
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
                _ => {}
            }

            // Recurse
            if cursor.goto_first_child() {
                self.collect_rust_deps(cursor, content, imports, exports);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_python_imports() {
        let mut extractor = DepsExtractor::new();
        let content = r#"
import os
import json as j
from pathlib import Path
from typing import Optional, List

def foo():
    pass

class Bar:
    pass
"#;
        let result = extractor.extract(&PathBuf::from("test.py"), content);

        assert!(result.imports.len() >= 3);
        assert!(result.exports.iter().any(|e| e.name == "foo"));
        assert!(result.exports.iter().any(|e| e.name == "Bar"));
    }

    #[test]
    fn test_rust_imports() {
        let mut extractor = DepsExtractor::new();
        let content = r#"
use std::path::Path;
use std::collections::{HashMap, HashSet};

pub fn foo() {}

pub struct Bar {}
"#;
        let result = extractor.extract(&PathBuf::from("test.rs"), content);

        assert!(result.imports.len() >= 2);
        assert!(result.exports.iter().any(|e| e.name == "foo"));
        assert!(result.exports.iter().any(|e| e.name == "Bar"));
    }
}
