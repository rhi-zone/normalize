//! Module dependency extraction.
//!
//! Extracts imports and exports from source files.

use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::{
    Export, Import, Language, SymbolKind, Visibility, support_for_grammar, support_for_path,
};
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// A re-export statement (export * from './module' or export { foo } from './module')
#[derive(Debug, Clone)]
pub struct ReExport {
    pub module: String,
    pub names: Vec<String>, // Empty for "export * from", specific names for "export { x } from"
    pub is_star: bool,      // true for "export * from"
    #[allow(dead_code)] // Consistent with Import/Export, useful for diagnostics
    pub line: usize,
}

/// Extracted dependencies (without file context)
struct ExtractedDeps {
    imports: Vec<Import>,
    exports: Vec<Export>,
    reexports: Vec<ReExport>,
}

/// Dependency information for a file.
pub struct DepsResult {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub reexports: Vec<ReExport>,
    /// Source file path, for context in downstream consumers.
    pub file_path: String,
}

/// Extract imports, exports, and re-exports from a source file.
pub fn extract_deps(path: &Path, content: &str) -> DepsResult {
    DepsExtractor.extract(path, content)
}

struct DepsExtractor;

impl DepsExtractor {
    fn extract(&self, path: &Path, content: &str) -> DepsResult {
        let support = support_for_path(path);

        let extracted = match support {
            // JS/TS need special handling for re-exports
            Some(s) if s.grammar_name() == "javascript" => self.extract_javascript(content),
            Some(s) if s.grammar_name() == "typescript" => self.extract_typescript(content),
            Some(s) if s.grammar_name() == "tsx" => self.extract_tsx(content),
            // All other languages use trait-based extraction
            Some(s) => self.extract_with_trait(content, s),
            None => ExtractedDeps {
                imports: Vec::new(),
                exports: Vec::new(),
                reexports: Vec::new(),
            },
        };

        DepsResult {
            imports: extracted.imports,
            exports: extracted.exports,
            reexports: extracted.reexports,
            file_path: path.to_string_lossy().to_string(),
        }
    }

    /// Extract exports from a parsed tree using the language's tags.scm query.
    ///
    /// Finds all `@definition.*` captures, checks visibility via `get_visibility()`,
    /// and maps the capture name to a `SymbolKind`.
    fn extract_exports_from_tags(
        tree: &tree_sitter::Tree,
        content: &str,
        support: &dyn Language,
        grammar_name: &str,
    ) -> Vec<Export> {
        let loader = grammar_loader();
        let tags_query_str = match loader.get_tags(grammar_name) {
            Some(q) => q,
            None => return Vec::new(),
        };
        let ts_lang = match loader.get(grammar_name) {
            Some(l) => l,
            None => return Vec::new(),
        };
        let query = match tree_sitter::Query::new(&ts_lang, &tags_query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };

        let capture_names = query.capture_names().to_vec();
        let root = tree.root_node();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&query, root, content.as_bytes());

        let mut exports = Vec::new();
        while let Some(m) = matches.next() {
            let mut def_node = None;
            let mut def_kind_str = "";
            for cap in m.captures {
                let cn = &capture_names[cap.index as usize];
                if cn.starts_with("definition.") {
                    def_node = Some(cap.node);
                    def_kind_str = cn;
                }
            }
            let Some(node) = def_node else { continue };

            if support.get_visibility(&node, content) != Visibility::Public {
                continue;
            }

            let name = match support.node_name(&node, content) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let kind = match def_kind_str {
                "definition.function" | "definition.method" | "definition.macro" => {
                    SymbolKind::Function
                }
                "definition.class" => SymbolKind::Class,
                "definition.interface" => SymbolKind::Interface,
                "definition.module" => SymbolKind::Module,
                "definition.type" => SymbolKind::Type,
                "definition.constant" => SymbolKind::Constant,
                "definition.var" => SymbolKind::Variable,
                _ => continue,
            };

            exports.push(Export {
                name,
                kind,
                line: node.start_position().row + 1,
            });
        }

        exports
    }

    /// Extract using the Language trait
    fn extract_with_trait(&self, content: &str, support: &dyn Language) -> ExtractedDeps {
        let grammar_name = support.grammar_name();
        let tree = match parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => {
                return ExtractedDeps {
                    imports: Vec::new(),
                    exports: Vec::new(),
                    reexports: Vec::new(),
                };
            }
        };

        let loader = grammar_loader();
        let imports = loader
            .get_imports(grammar_name)
            .and_then(|query_str| {
                // Query-first: use the .scm file; None means fall back to trait
                Self::collect_imports_from_query(&tree, content, grammar_name, &query_str, &loader)
            })
            .unwrap_or_else(|| {
                // Fallback: walk AST with Language trait (no .scm or query failed to compile)
                let mut imports = Vec::new();
                let root = tree.root_node();
                let mut cursor = root.walk();
                Self::collect_imports_with_trait(&mut cursor, content, support, &mut imports);
                imports
            });

        let exports = Self::extract_exports_from_tags(&tree, content, support, grammar_name);

        ExtractedDeps {
            imports,
            exports,
            reexports: Vec::new(),
        }
    }

    /// Extract imports from an imports.scm query.
    ///
    /// Returns `None` when the grammar or query is unavailable, or when the query produced
    /// no usable import paths (triggering trait fallback in both cases).
    /// Returns `Some(vec)` when the query extracted at least one import with a non-empty path.
    ///
    /// Captures used:
    /// - `@import`      — the whole import node (provides the line number anchor)
    /// - `@import.path` — the module path (quotes stripped if present)
    /// - `@import.name` — a single imported name (may repeat per match for multi-name imports)
    /// - `@import.alias`— alias for the name or path
    /// - `@import.glob` — presence means `is_wildcard = true`
    ///
    /// Multiple matches may share the same `@import` node (e.g. Rust `use path::{A, B}` emits
    /// one match per name). They are aggregated into a single `Import` by source position.
    fn collect_imports_from_query(
        tree: &tree_sitter::Tree,
        content: &str,
        grammar_name: &str,
        query_str: &str,
        loader: &normalize_languages::GrammarLoader,
    ) -> Option<Vec<Import>> {
        let ts_lang = loader.get(grammar_name)?;
        let query = tree_sitter::Query::new(&ts_lang, query_str).ok()?;

        let capture_names = query.capture_names().to_vec();
        let root = tree.root_node();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&query, root, content.as_bytes());

        // Use an ordered map (keyed by byte offset of @import node) so we can aggregate
        // multiple matches that belong to the same import statement.
        let mut seen: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut result: Vec<Import> = Vec::new();

        while let Some(m) = matches.next() {
            let mut anchor_byte: Option<usize> = None;
            let mut anchor_line = 0usize;
            let mut path: Option<String> = None;
            let mut name: Option<String> = None;
            let mut alias: Option<String> = None;
            let mut is_glob = false;

            for cap in m.captures {
                let cn = &capture_names[cap.index as usize];
                let text = content[cap.node.byte_range()].to_string();
                match *cn {
                    "import" => {
                        anchor_byte = Some(cap.node.start_byte());
                        anchor_line = cap.node.start_position().row + 1;
                    }
                    "import.path" => {
                        // Strip surrounding quotes if present (Go, JS/TS use quoted strings)
                        path = Some(
                            text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
                                .to_string(),
                        );
                    }
                    "import.name" => {
                        name = Some(text);
                    }
                    "import.alias" => {
                        alias = Some(text);
                    }
                    "import.glob" => {
                        is_glob = true;
                    }
                    _ => {}
                }
            }

            // Determine the grouping key: anchor byte if we have one, else path byte start
            let key = match anchor_byte {
                Some(b) => b,
                None => continue, // No @import capture — skip malformed match
            };

            let module = path.unwrap_or_default();
            let is_relative = module.starts_with('.');

            if let Some(&idx) = seen.get(&key) {
                // This is an additional name for an existing import (e.g. use path::{A, B})
                if let Some(name) = name {
                    result[idx].names.push(name);
                }
                if alias.is_some() {
                    result[idx].alias = alias;
                }
                if is_glob {
                    result[idx].is_wildcard = true;
                }
            } else {
                // Skip sentinel matches with no usable path info (e.g. Scala's @import-only query)
                if module.is_empty() && name.is_none() && !is_glob {
                    continue;
                }
                let mut imp = Import {
                    module,
                    names: Vec::new(),
                    alias,
                    is_wildcard: is_glob,
                    is_relative,
                    line: anchor_line,
                };
                if let Some(name) = name {
                    imp.names.push(name);
                }
                seen.insert(key, result.len());
                result.push(imp);
            }
        }

        // Return None when no usable imports found — signals caller to use trait fallback
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn collect_imports_with_trait(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        support: &dyn Language,
        imports: &mut Vec<Import>,
    ) {
        loop {
            let node = cursor.node();

            // Check for embedded content (e.g., <script> in Vue/Svelte/HTML)
            if let Some(embedded) = support
                .as_embedded()
                .and_then(|e| e.embedded_content(&node, content))
                && let Some(sub_lang) = support_for_grammar(embedded.grammar)
                && let Some(sub_tree) = parse_with_grammar(embedded.grammar, &embedded.content)
            {
                let mut sub_imports = Vec::new();
                let sub_root = sub_tree.root_node();
                let mut sub_cursor = sub_root.walk();
                Self::collect_imports_with_trait(
                    &mut sub_cursor,
                    &embedded.content,
                    sub_lang,
                    &mut sub_imports,
                );

                // Collect exports from embedded content via tags
                let sub_exports = Self::extract_exports_from_tags(
                    &sub_tree,
                    &embedded.content,
                    sub_lang,
                    embedded.grammar,
                );
                let _ = sub_exports; // Embedded exports are not propagated (only imports are)

                // Adjust line numbers for embedded content offset
                for mut imp in sub_imports {
                    imp.line += embedded.start_line - 1;
                    imports.push(imp);
                }
                // Don't descend into embedded nodes - we've already processed them
                if cursor.goto_next_sibling() {
                    continue;
                }
                break;
            }

            // Extract imports from this node
            let node_imports = support.extract_imports(&node, content);
            imports.extend(node_imports);

            // Recurse into children
            if cursor.goto_first_child() {
                Self::collect_imports_with_trait(cursor, content, support, imports);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn extract_typescript(&self, content: &str) -> ExtractedDeps {
        let tree = match parse_with_grammar("typescript", content) {
            Some(t) => t,
            None => {
                return ExtractedDeps {
                    imports: Vec::new(),
                    exports: Vec::new(),
                    reexports: Vec::new(),
                };
            }
        };
        self.extract_js_ts_deps(&tree, content)
    }

    fn extract_tsx(&self, content: &str) -> ExtractedDeps {
        let tree = match parse_with_grammar("tsx", content) {
            Some(t) => t,
            None => {
                return ExtractedDeps {
                    imports: Vec::new(),
                    exports: Vec::new(),
                    reexports: Vec::new(),
                };
            }
        };
        self.extract_js_ts_deps(&tree, content)
    }

    fn extract_javascript(&self, content: &str) -> ExtractedDeps {
        let tree = match parse_with_grammar("javascript", content) {
            Some(t) => t,
            None => {
                return ExtractedDeps {
                    imports: Vec::new(),
                    exports: Vec::new(),
                    reexports: Vec::new(),
                };
            }
        };
        self.extract_js_ts_deps(&tree, content)
    }

    /// Shared extraction for JavaScript/TypeScript AST
    fn extract_js_ts_deps(&self, tree: &tree_sitter::Tree, content: &str) -> ExtractedDeps {
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut reexports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        Self::collect_js_ts_deps(
            &mut cursor,
            content,
            &mut imports,
            &mut exports,
            &mut reexports,
        );
        ExtractedDeps {
            imports,
            exports,
            reexports,
        }
    }

    fn collect_js_ts_deps(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        imports: &mut Vec<Import>,
        exports: &mut Vec<Export>,
        reexports: &mut Vec<ReExport>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // import { foo, bar } from './module'
                // import foo from './module'
                // import * as foo from './module'
                "import_statement" => {
                    let mut module = String::new();
                    let mut names = Vec::new();

                    for i in 0..node.child_count() as u32 {
                        if let Some(child) = node.child(i) {
                            match child.kind() {
                                "string" | "string_fragment" => {
                                    // Extract module path (remove quotes)
                                    let text = &content[child.byte_range()];
                                    module =
                                        text.trim_matches(|c| c == '"' || c == '\'').to_string();
                                }
                                "import_clause" => {
                                    // Extract imported names
                                    Self::collect_import_names(child, content, &mut names);
                                }
                                _ => {}
                            }
                        }
                    }

                    if !module.is_empty() {
                        let is_relative = module.starts_with('.');
                        imports.push(Import {
                            module,
                            names,
                            alias: None,
                            is_wildcard: false,
                            is_relative,
                            line: node.start_position().row + 1,
                        });
                    }
                }
                // export function foo() {}
                // export class Bar {}
                // export const baz = ...
                // export * from './module'
                // export { foo, bar } from './module'
                // export * as helpers from './helpers'
                "export_statement" => {
                    // Check if this is a re-export (has a source module)
                    let mut source_module = None;
                    let mut is_star = false;
                    let mut named_exports: Vec<String> = Vec::new();

                    for i in 0..node.child_count() as u32 {
                        if let Some(child) = node.child(i) {
                            match child.kind() {
                                "string" => {
                                    // The source module in 'export ... from "module"'
                                    let text = &content[child.byte_range()];
                                    source_module = Some(
                                        text.trim_matches(|c| c == '"' || c == '\'').to_string(),
                                    );
                                }
                                "*" => {
                                    // export * from './module'
                                    is_star = true;
                                }
                                "namespace_export" => {
                                    // export * as foo from './module'
                                    is_star = true;
                                }
                                "export_clause" => {
                                    // export { foo, bar } from './module'
                                    Self::collect_export_clause_names(
                                        child,
                                        content,
                                        &mut named_exports,
                                    );
                                }
                                "function_declaration" | "generator_function_declaration" => {
                                    if let Some(name_node) = child.child_by_field_name("name") {
                                        exports.push(Export {
                                            name: content[name_node.byte_range()].to_string(),
                                            kind: SymbolKind::Function,
                                            line: node.start_position().row + 1,
                                        });
                                    }
                                }
                                "class_declaration" => {
                                    if let Some(name_node) = child.child_by_field_name("name") {
                                        exports.push(Export {
                                            name: content[name_node.byte_range()].to_string(),
                                            kind: SymbolKind::Class,
                                            line: node.start_position().row + 1,
                                        });
                                    }
                                }
                                "lexical_declaration" => {
                                    // export const foo = ..., bar = ...
                                    Self::collect_variable_names(
                                        child,
                                        content,
                                        exports,
                                        node.start_position().row + 1,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }

                    // If we found a source module, this is a re-export
                    if let Some(module) = source_module {
                        reexports.push(ReExport {
                            module,
                            names: named_exports,
                            is_star,
                            line: node.start_position().row + 1,
                        });
                    }
                }
                // Top-level function/class (could be exported via export default later)
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        if !name.starts_with('_') {
                            exports.push(Export {
                                name,
                                kind: SymbolKind::Function,
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
                "class_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = content[name_node.byte_range()].to_string();
                        if !name.starts_with('_') {
                            exports.push(Export {
                                name,
                                kind: SymbolKind::Class,
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
                // const x = require('./module')
                // let { a, b } = require('./module')
                // var x = require('./module')
                "lexical_declaration" | "variable_declaration" => {
                    let line = node.start_position().row + 1;
                    for i in 0..node.child_count() as u32 {
                        if let Some(decl) = node.child(i)
                            && decl.kind() == "variable_declarator"
                            && let Some(value) = decl.child_by_field_name("value")
                            && let Some(mut imp) = Self::extract_require_call(value, content, line)
                        {
                            if let Some(name_node) = decl.child_by_field_name("name") {
                                match name_node.kind() {
                                    "identifier" => {
                                        imp.names =
                                            vec![content[name_node.byte_range()].to_string()];
                                    }
                                    "object_pattern" => {
                                        Self::collect_destructure_names(
                                            name_node,
                                            content,
                                            &mut imp.names,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                            imports.push(imp);
                        }
                    }
                }
                // require('./side-effect') — bare require, no binding
                "expression_statement" => {
                    let line = node.start_position().row + 1;
                    if let Some(child) = node.child(0)
                        && let Some(imp) = Self::extract_require_call(child, content, line)
                    {
                        imports.push(imp);
                    }
                }
                _ => {}
            }

            // Recurse into children
            if cursor.goto_first_child() {
                Self::collect_js_ts_deps(cursor, content, imports, exports, reexports);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract a require('module') call from a call_expression node.
    /// Returns Some(Import) if this is a require(string) call, None otherwise.
    fn extract_require_call(node: tree_sitter::Node, content: &str, line: usize) -> Option<Import> {
        if node.kind() != "call_expression" {
            return None;
        }
        let func = node.child_by_field_name("function")?;
        if func.kind() != "identifier" || &content[func.byte_range()] != "require" {
            return None;
        }
        let args = node.child_by_field_name("arguments")?;
        let module = Self::extract_string_from_args(args, content)?;
        Some(Import {
            is_relative: module.starts_with('.'),
            module,
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            line,
        })
    }

    /// Extract the first string literal from an arguments node.
    fn extract_string_from_args(args: tree_sitter::Node, content: &str) -> Option<String> {
        for i in 0..args.child_count() as u32 {
            let arg = args.child(i)?;
            match arg.kind() {
                "string" => {
                    for j in 0..arg.child_count() as u32 {
                        if let Some(frag) = arg.child(j)
                            && frag.kind() == "string_fragment"
                        {
                            return Some(content[frag.byte_range()].to_string());
                        }
                    }
                }
                "string_fragment" => {
                    return Some(content[arg.byte_range()].to_string());
                }
                _ => {}
            }
        }
        None
    }

    /// Collect bound names from a destructuring object pattern.
    /// Handles `{ a, b }` and `{ key: alias }`.
    fn collect_destructure_names(node: tree_sitter::Node, content: &str, names: &mut Vec<String>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "shorthand_property_identifier_pattern" | "identifier" => {
                    names.push(content[child.byte_range()].to_string());
                }
                "pair_pattern" => {
                    // { key: boundName } — use the bound name (value side)
                    if let Some(val) = child.child_by_field_name("value")
                        && val.kind() == "identifier"
                    {
                        names.push(content[val.byte_range()].to_string());
                    }
                }
                _ => {}
            }
        }
    }

    /// Collect names from export clause: export { foo, bar } from ...
    fn collect_export_clause_names(
        node: tree_sitter::Node,
        content: &str,
        names: &mut Vec<String>,
    ) {
        // Walk through children directly
        for i in 0..node.child_count() as u32 {
            if let Some(child) = node.child(i) {
                match child.kind() {
                    "export_specifier" => {
                        // { foo as bar } - get the first identifier (original name)
                        // or check for "name" field
                        if let Some(name) = child.child_by_field_name("name") {
                            names.push(content[name.byte_range()].to_string());
                        } else {
                            // Find first identifier child
                            for j in 0..child.child_count() as u32 {
                                if let Some(id) = child.child(j)
                                    && id.kind() == "identifier"
                                {
                                    names.push(content[id.byte_range()].to_string());
                                    break;
                                }
                            }
                        }
                    }
                    _ => {
                        // Recurse into other nodes
                        Self::collect_export_clause_names(child, content, names);
                    }
                }
            }
        }
    }

    fn collect_import_names(node: tree_sitter::Node, content: &str, names: &mut Vec<String>) {
        let mut cursor = node.walk();
        loop {
            let child = cursor.node();
            match child.kind() {
                "identifier" => {
                    names.push(content[child.byte_range()].to_string());
                }
                "import_specifier" => {
                    // { foo as bar } - we want "foo"
                    if let Some(name) = child.child_by_field_name("name") {
                        names.push(content[name.byte_range()].to_string());
                    }
                }
                "namespace_import" => {
                    // import * as foo - we want "foo"
                    for i in 0..child.child_count() as u32 {
                        if let Some(id) = child.child(i)
                            && id.kind() == "identifier"
                        {
                            names.push(content[id.byte_range()].to_string());
                        }
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_import_names(cursor.node(), content, names);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn collect_variable_names(
        node: tree_sitter::Node,
        content: &str,
        exports: &mut Vec<Export>,
        line: usize,
    ) {
        let mut cursor = node.walk();
        loop {
            let child = cursor.node();
            if child.kind() == "variable_declarator"
                && let Some(name_node) = child.child_by_field_name("name")
                && name_node.kind() == "identifier"
            {
                exports.push(Export {
                    name: content[name_node.byte_range()].to_string(),
                    kind: SymbolKind::Variable,
                    line,
                });
            }

            if cursor.goto_first_child() {
                Self::collect_variable_names(cursor.node(), content, exports, line);
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
        let result = extract_deps(&PathBuf::from("test.py"), content);

        assert!(result.imports.len() >= 3);
        assert!(result.exports.iter().any(|e| e.name == "foo"));
        assert!(result.exports.iter().any(|e| e.name == "Bar"));
    }

    #[test]
    fn test_rust_imports() {
        let content = r#"
use std::path::Path;
use std::collections::{HashMap, HashSet};

pub fn foo() {}

pub struct Bar {}
"#;
        let result = extract_deps(&PathBuf::from("test.rs"), content);

        assert!(result.imports.len() >= 2);
        assert!(result.exports.iter().any(|e| e.name == "foo"));
        assert!(result.exports.iter().any(|e| e.name == "Bar"));
    }

    #[test]
    fn test_typescript_imports() {
        let content = r#"
import { foo, bar } from './utils';
import React from 'react';
import * as helpers from '../helpers';

export function greet(name: string): string {
    return `Hello, ${name}`;
}

export class User {
    name: string;
}

export const VERSION = "1.0.0";
"#;
        let result = extract_deps(&PathBuf::from("test.ts"), content);

        assert!(result.imports.len() >= 2);
        assert!(result.imports.iter().any(|i| i.module == "./utils"));
        assert!(result.exports.iter().any(|e| e.name == "greet"));
        assert!(result.exports.iter().any(|e| e.name == "User"));
    }

    #[test]
    fn test_typescript_barrel_reexports() {
        let content = r#"
export * from './utils';
export * as helpers from './helpers';
export { foo, bar } from './specific';
"#;
        let result = extract_deps(&PathBuf::from("index.ts"), content);

        assert_eq!(result.reexports.len(), 3);

        // Star re-export
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let star = result
            .reexports
            .iter()
            .find(|r| r.module == "./utils")
            .unwrap();
        assert!(star.is_star);
        assert!(star.names.is_empty());

        // Namespace re-export (export * as helpers)
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let namespace = result
            .reexports
            .iter()
            .find(|r| r.module == "./helpers")
            .unwrap();
        assert!(namespace.is_star);

        // Named re-export
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let named = result
            .reexports
            .iter()
            .find(|r| r.module == "./specific")
            .unwrap();
        assert!(!named.is_star);
        assert!(named.names.contains(&"foo".to_string()));
        assert!(named.names.contains(&"bar".to_string()));
    }

    #[test]
    fn test_go_imports() {
        let content = r#"
package main

import "fmt"

import (
    "os"
    "path/filepath"
    alias "github.com/user/pkg"
)

func main() {}

func PublicFunc() {}

func privateFunc() {}

type PublicType struct {}

type privateType struct {}

const PublicConst = 1

var PublicVar = "hello"
"#;
        let result = extract_deps(&PathBuf::from("main.go"), content);

        // Check imports
        assert!(result.imports.iter().any(|i| i.module == "fmt"));
        assert!(result.imports.iter().any(|i| i.module == "os"));
        assert!(result.imports.iter().any(|i| i.module == "path/filepath"));
        assert!(
            result
                .imports
                .iter()
                .any(|i| i.module == "github.com/user/pkg" && i.alias == Some("alias".to_string()))
        );

        // Check exports (only uppercase names are exported in Go)
        assert!(result.exports.iter().any(|e| e.name == "PublicFunc"));
        assert!(result.exports.iter().any(|e| e.name == "PublicType"));
        assert!(result.exports.iter().any(|e| e.name == "PublicConst"));
        assert!(result.exports.iter().any(|e| e.name == "PublicVar"));

        // Private items should NOT be exported
        assert!(!result.exports.iter().any(|e| e.name == "main"));
        assert!(!result.exports.iter().any(|e| e.name == "privateFunc"));
        assert!(!result.exports.iter().any(|e| e.name == "privateType"));
    }

    #[test]
    fn test_vue_embedded_imports() {
        let content = r#"
<template>
  <div>{{ message }}</div>
</template>

<script lang="ts">
import { ref, computed } from 'vue';
import { useStore } from './store';

export function greet(name: string): string {
  return `Hello, ${name}`;
}

const message = ref('Hello World');
</script>
"#;
        let result = extract_deps(&PathBuf::from("App.vue"), content);

        // Check imports from embedded script
        assert!(
            !result.imports.is_empty(),
            "Should extract imports from Vue script: {:?}",
            result.imports
        );
        assert!(
            result.imports.iter().any(|i| i.module == "vue"),
            "Should have vue import"
        );
        assert!(
            result
                .imports
                .iter()
                .any(|i| i.module == "./store" && i.is_relative),
            "Should have relative store import"
        );

        // Verify line numbers are correctly offset
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let vue_import = result.imports.iter().find(|i| i.module == "vue").unwrap();
        assert!(
            vue_import.line >= 7,
            "Vue import should be on line 7 or later (was {})",
            vue_import.line
        );
    }

    #[test]
    fn test_html_embedded_imports() {
        let content = r#"
<!DOCTYPE html>
<html>
<body>
  <script type="module">
    import { init } from './app.js';

    function main() {
      init();
    }
  </script>
</body>
</html>
"#;
        let result = extract_deps(&PathBuf::from("index.html"), content);

        // Check imports from embedded script
        assert!(
            !result.imports.is_empty(),
            "Should extract imports from HTML script"
        );
        assert!(
            result.imports.iter().any(|i| i.module == "./app.js"),
            "Should have app.js import"
        );
    }

    #[test]
    fn test_commonjs_require_imports() {
        let content = r#"
const path = require('path');
const { readFile, writeFile } = require('fs');
const { join: joinPath } = require('path');
const express = require('express');
require('./side-effect');

module.exports = { path, express };
"#;
        let result = extract_deps(&PathBuf::from("test.js"), content);

        // Simple binding: const x = require(...)
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let path_import = result
            .imports
            .iter()
            .find(|i| i.module == "path" && i.names.contains(&"path".to_string()));
        assert!(
            path_import.is_some(),
            "Should extract const path = require('path')"
        );

        // Destructured: const { a, b } = require(...)
        let fs_import = result.imports.iter().find(|i| i.module == "fs");
        assert!(fs_import.is_some(), "Should extract require('fs')");
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let fs_import = fs_import.unwrap();
        assert!(
            fs_import.names.contains(&"readFile".to_string()),
            "Should extract destructured name 'readFile'"
        );
        assert!(
            fs_import.names.contains(&"writeFile".to_string()),
            "Should extract destructured name 'writeFile'"
        );

        // Aliased destructuring: { join: joinPath }
        let join_import = result
            .imports
            .iter()
            .find(|i| i.module == "path" && i.names.contains(&"joinPath".to_string()));
        assert!(
            join_import.is_some(),
            "Should extract aliased destructure {{ join: joinPath }}"
        );

        // Bare require (side-effect)
        assert!(
            result.imports.iter().any(|i| i.module == "./side-effect"),
            "Should extract bare require('./side-effect')"
        );
    }

    #[test]
    fn test_commonjs_require_in_typescript() {
        let content = r#"
const fs = require('fs');
const { EventEmitter } = require('events');
import { Something } from './es-module';
"#;
        let result = extract_deps(&PathBuf::from("test.ts"), content);

        assert!(
            result.imports.iter().any(|i| i.module == "fs"),
            "TypeScript: should extract const fs = require('fs')"
        );
        assert!(
            result.imports.iter().any(|i| i.module == "events"),
            "TypeScript: should extract destructured require('events')"
        );
        assert!(
            result.imports.iter().any(|i| i.module == "./es-module"),
            "TypeScript: should still extract ES6 imports"
        );
    }
}
