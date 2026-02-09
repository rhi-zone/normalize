//! Shared ECMAScript (JavaScript/TypeScript) support functions.
//!
//! This module contains common logic shared between JavaScript, TypeScript, and TSX.
//! Each language struct delegates to these functions for DRY implementation.

use crate::{Export, Import, Symbol, SymbolKind, Visibility};
use tree_sitter::Node;

// ============================================================================
// Node kind constants
// ============================================================================

pub const JS_CONTAINER_KINDS: &[&str] = &["class_declaration", "class"];
pub const TS_CONTAINER_KINDS: &[&str] = &["class_declaration", "class", "interface_declaration"];

pub const JS_FUNCTION_KINDS: &[&str] = &[
    "function_declaration",
    "method_definition",
    "generator_function_declaration",
];
pub const TS_FUNCTION_KINDS: &[&str] = &[
    "function_declaration",
    "method_definition",
    "method_signature", // Interface methods
];

pub const JS_TYPE_KINDS: &[&str] = &["class_declaration"];
pub const TS_TYPE_KINDS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "type_alias_declaration",
    "enum_declaration",
];

pub const IMPORT_KINDS: &[&str] = &["import_statement"];
pub const PUBLIC_SYMBOL_KINDS: &[&str] = &["export_statement"];

pub const SCOPE_CREATING_KINDS: &[&str] = &[
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "try_statement",
    "catch_clause",
    "switch_statement",
    "arrow_function",
];

pub const CONTROL_FLOW_KINDS: &[&str] = &[
    "if_statement",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_statement",
    "try_statement",
    "return_statement",
    "break_statement",
    "continue_statement",
    "throw_statement",
];

pub const COMPLEXITY_NODES: &[&str] = &[
    "if_statement",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_case",
    "catch_clause",
    "ternary_expression",
    "binary_expression",
];

pub const NESTING_NODES: &[&str] = &[
    "if_statement",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_statement",
    "try_statement",
    "function_declaration",
    "method_definition",
    "class_declaration",
];

// ============================================================================
// Symbol extraction
// ============================================================================

/// Extract a function/method symbol from a node.
pub fn extract_function(node: &Node, content: &str, in_container: bool, name: &str) -> Symbol {
    let params = node
        .child_by_field_name("parameters")
        .map(|p| content[p.byte_range()].to_string())
        .unwrap_or_else(|| "()".to_string());

    let signature = if node.kind() == "method_definition" {
        format!("{}{}", name, params)
    } else {
        format!("function {}{}", name, params)
    };

    // Check for explicit override modifier (TypeScript)
    let is_override = {
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        children
            .iter()
            .any(|child| child.kind() == "override_modifier")
    };

    Symbol {
        name: name.to_string(),
        kind: if in_container {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        },
        signature,
        docstring: None,
        attributes: Vec::new(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: Visibility::Public,
        children: Vec::new(),
        is_interface_impl: is_override,
        implements: Vec::new(),
    }
}

/// Extract a class or interface container symbol from a node.
pub fn extract_container(node: &Node, content: &str, name: &str) -> Symbol {
    let (kind, keyword) = if node.kind() == "interface_declaration" {
        (SymbolKind::Interface, "interface")
    } else {
        (SymbolKind::Class, "class")
    };

    // Extract implements/extends clauses for semantic interface detection
    let mut implements = Vec::new();
    // Find class_heritage child node (not a field)
    for i in 0..node.child_count() as u32 {
        if let Some(heritage) = node.child(i)
            && heritage.kind() == "class_heritage"
        {
            for j in 0..heritage.child_count() as u32 {
                if let Some(clause) = heritage.child(j) {
                    if clause.kind() == "extends_clause" || clause.kind() == "implements_clause" {
                        // TypeScript: heritage > extends_clause/implements_clause > type_identifier
                        for k in 0..clause.child_count() as u32 {
                            if let Some(type_node) = clause.child(k)
                                && (type_node.kind() == "type_identifier"
                                    || type_node.kind() == "identifier")
                            {
                                implements.push(content[type_node.byte_range()].to_string());
                            }
                        }
                    } else if clause.kind() == "type_identifier" || clause.kind() == "identifier" {
                        // JavaScript: heritage > identifier (no clause wrapper)
                        implements.push(content[clause.byte_range()].to_string());
                    }
                }
            }
        }
    }

    Symbol {
        name: name.to_string(),
        kind,
        signature: format!("{} {}", keyword, name),
        docstring: None,
        attributes: Vec::new(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: Visibility::Public,
        children: Vec::new(),
        is_interface_impl: false,
        implements,
    }
}

/// Extract a TypeScript type symbol (interface, type alias, enum).
pub fn extract_type(node: &Node, name: &str) -> Option<Symbol> {
    let (kind, keyword) = match node.kind() {
        "interface_declaration" => (SymbolKind::Interface, "interface"),
        "type_alias_declaration" => (SymbolKind::Type, "type"),
        "enum_declaration" => (SymbolKind::Enum, "enum"),
        "class_declaration" => (SymbolKind::Class, "class"),
        _ => return None,
    };

    Some(Symbol {
        name: name.to_string(),
        kind,
        signature: format!("{} {}", keyword, name),
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

// ============================================================================
// Import/Export extraction
// ============================================================================

/// Extract imports from an import_statement node.
pub fn extract_imports(node: &Node, content: &str) -> Vec<Import> {
    if node.kind() != "import_statement" {
        return Vec::new();
    }

    let line = node.start_position().row + 1;
    let mut module = String::new();
    let mut names = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" | "string_fragment" => {
                let text = &content[child.byte_range()];
                module = text.trim_matches(|c| c == '"' || c == '\'').to_string();
            }
            "import_clause" => {
                collect_import_names(&child, content, &mut names);
            }
            _ => {}
        }
    }

    if module.is_empty() {
        return Vec::new();
    }

    vec![Import {
        module: module.clone(),
        names,
        alias: None,
        is_wildcard: false,
        is_relative: module.starts_with('.'),
        line,
    }]
}

/// Format an import as JavaScript/TypeScript source code.
pub fn format_import(import: &Import, names: Option<&[&str]>) -> String {
    let names_to_use: Vec<&str> = names
        .map(|n| n.to_vec())
        .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());

    if import.is_wildcard {
        format!("import * from '{}';", import.module)
    } else if names_to_use.is_empty() {
        format!("import '{}';", import.module)
    } else if names_to_use.len() == 1 {
        format!("import {{ {} }} from '{}';", names_to_use[0], import.module)
    } else {
        format!(
            "import {{ {} }} from '{}';",
            names_to_use.join(", "),
            import.module
        )
    }
}

fn collect_import_names(import_clause: &Node, content: &str, names: &mut Vec<String>) {
    let mut cursor = import_clause.walk();
    for child in import_clause.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                // Default import: import foo from './module'
                names.push(content[child.byte_range()].to_string());
            }
            "named_imports" => {
                // { foo, bar }
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "import_specifier"
                        && let Some(name_node) = inner.child_by_field_name("name")
                    {
                        names.push(content[name_node.byte_range()].to_string());
                    }
                }
            }
            "namespace_import" => {
                // import * as foo
                if let Some(name_node) = child.child_by_field_name("name") {
                    names.push(format!("* as {}", &content[name_node.byte_range()]));
                }
            }
            _ => {}
        }
    }
}

/// Extract exports from an export_statement node.
pub fn extract_public_symbols(node: &Node, content: &str) -> Vec<Export> {
    if node.kind() != "export_statement" {
        return Vec::new();
    }

    let line = node.start_position().row + 1;
    let mut exports = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    exports.push(Export {
                        name: content[name_node.byte_range()].to_string(),
                        kind: SymbolKind::Function,
                        line,
                    });
                }
            }
            "class_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    exports.push(Export {
                        name: content[name_node.byte_range()].to_string(),
                        kind: SymbolKind::Class,
                        line,
                    });
                }
            }
            "lexical_declaration" => {
                // export const foo = ...
                let mut decl_cursor = child.walk();
                for decl_child in child.children(&mut decl_cursor) {
                    if decl_child.kind() == "variable_declarator"
                        && let Some(name_node) = decl_child.child_by_field_name("name")
                    {
                        exports.push(Export {
                            name: content[name_node.byte_range()].to_string(),
                            kind: SymbolKind::Variable,
                            line,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    exports
}
