//! Shared ECMAScript (JavaScript/TypeScript) support functions.
//!
//! This module contains common logic shared between JavaScript, TypeScript, and TSX.
//! Each language struct delegates to these functions for DRY implementation.

use crate::{ImplementsInfo, Import};
use tree_sitter::Node;

// ============================================================================
// Semantic hook helpers (for Language trait build_signature / extract_implements)
// ============================================================================

/// Build signature for a JS/TS function, method, or class node.
pub fn build_signature(node: &Node, content: &str, name: &str) -> String {
    match node.kind() {
        "method_definition" | "method_signature" => {
            let params = node
                .child_by_field_name("parameters")
                .map(|p| content[p.byte_range()].to_string())
                .unwrap_or_else(|| "()".to_string());
            format!("{}{}", name, params)
        }
        "function_declaration" | "generator_function_declaration" => {
            let params = node
                .child_by_field_name("parameters")
                .map(|p| content[p.byte_range()].to_string())
                .unwrap_or_else(|| "()".to_string());
            format!("function {}{}", name, params)
        }
        "class_declaration" | "class" => format!("class {}", name),
        "interface_declaration" => format!("interface {}", name),
        "type_alias_declaration" => format!("type {}", name),
        "enum_declaration" => format!("enum {}", name),
        _ => {
            let text = &content[node.byte_range()];
            text.lines().next().unwrap_or(text).trim().to_string()
        }
    }
}

/// Extract implements/extends list for a JS/TS class or interface node.
pub fn extract_implements(node: &Node, content: &str) -> ImplementsInfo {
    let mut implements = Vec::new();
    for i in 0..node.child_count() as u32 {
        if let Some(heritage) = node.child(i)
            && heritage.kind() == "class_heritage"
        {
            for j in 0..heritage.child_count() as u32 {
                if let Some(clause) = heritage.child(j) {
                    if clause.kind() == "extends_clause" || clause.kind() == "implements_clause" {
                        for k in 0..clause.child_count() as u32 {
                            if let Some(type_node) = clause.child(k)
                                && (type_node.kind() == "type_identifier"
                                    || type_node.kind() == "identifier")
                            {
                                implements.push(content[type_node.byte_range()].to_string());
                            }
                        }
                    } else if clause.kind() == "type_identifier" || clause.kind() == "identifier" {
                        implements.push(content[clause.byte_range()].to_string());
                    }
                }
            }
        }
    }
    ImplementsInfo {
        is_interface: false,
        implements,
    }
}

// ============================================================================
// Docstring extraction
// ============================================================================

/// Extract a JSDoc comment (`/** ... */`) preceding a node.
///
/// Walks backwards through siblings looking for a `comment` starting with `/**`.
pub fn extract_jsdoc(node: &Node, content: &str) -> Option<String> {
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        match sibling.kind() {
            "comment" => {
                let text = &content[sibling.byte_range()];
                if text.starts_with("/**") {
                    return Some(clean_block_doc_comment(text));
                }
                return None;
            }
            "decorator" | "export_statement" => {}
            _ => return None,
        }
        prev = sibling.prev_sibling();
    }
    None
}

/// Clean a `/** ... */` block doc comment into plain text.
fn clean_block_doc_comment(text: &str) -> String {
    let lines: Vec<&str> = text
        .strip_prefix("/**")
        .unwrap_or(text)
        .strip_suffix("*/")
        .unwrap_or(text)
        .lines()
        .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join(" ")
}

/// Extract decorator attributes (`@decorator`) preceding a node.
///
/// Walks backwards through siblings looking for `decorator` nodes.
pub fn extract_decorators(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "decorator" {
            attrs.insert(0, content[sibling.byte_range()].to_string());
        } else if sibling.kind() == "comment" {
            // Skip comments between decorators and declaration
        } else {
            break;
        }
        prev = sibling.prev_sibling();
    }
    attrs
}

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
// Import/export extraction
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
