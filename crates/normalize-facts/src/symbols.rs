use crate::extract::{ExtractOptions, Extractor};
use crate::parsers;
use normalize_facts_core::TypeRef;
use normalize_facts_core::TypeRefKind;
use normalize_languages::{Symbol as LangSymbol, support_for_path};
use std::path::Path;
use streaming_iterator::StreamingIterator;

// Re-export for use by other modules in this crate
pub use normalize_facts_core::{FlatImport, FlatSymbol};

pub struct SymbolParser {
    extractor: Extractor,
    // Keep for import parsing and call graph analysis
}

impl Default for SymbolParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolParser {
    pub fn new() -> Self {
        Self {
            extractor: Extractor::with_options(ExtractOptions {
                include_private: true, // symbols.rs includes all symbols for indexing
            }),
        }
    }

    pub fn parse_file(&self, path: &Path, content: &str) -> Vec<FlatSymbol> {
        if support_for_path(path).is_none() {
            return Vec::new();
        }

        // Use shared extractor for symbol extraction
        let result = self.extractor.extract(path, content);

        // Flatten nested symbols
        let mut symbols = Vec::new();
        for sym in &result.symbols {
            Self::flatten_symbol(sym, None, &mut symbols);
        }
        symbols
    }

    /// Flatten a nested symbol into the flat list with parent references
    fn flatten_symbol(sym: &LangSymbol, parent: Option<&str>, symbols: &mut Vec<FlatSymbol>) {
        symbols.push(FlatSymbol {
            name: sym.name.clone(),
            kind: sym.kind,
            start_line: sym.start_line,
            end_line: sym.end_line,
            parent: parent.map(String::from),
            visibility: sym.visibility,
            attributes: sym.attributes.clone(),
            is_interface_impl: sym.is_interface_impl,
            implements: sym.implements.clone(),
        });

        // Recurse into children with current symbol as parent
        for child in &sym.children {
            Self::flatten_symbol(child, Some(&sym.name), symbols);
        }
    }

    /// Parse imports from any supported language file.
    /// Tries query-based extraction first; falls back to trait-based extraction.
    /// Returns a flattened list where each imported name gets its own FlatImport entry.
    pub fn parse_imports(&self, path: &Path, content: &str) -> Vec<FlatImport> {
        let support = match support_for_path(path) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let grammar_name = support.grammar_name();

        // Check if this language has import support (either via query or trait)
        let loader = normalize_languages::parsers::grammar_loader();
        if loader.get_imports(grammar_name).is_none() {
            return Vec::new();
        }
        let tree = match parsers::parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let root = tree.root_node();

        // Query-based extraction (compiled query is cached in the loader)
        if let Some(query_str) = loader.get_imports(grammar_name)
            && let Some(query) = loader.get_compiled_query(grammar_name, "imports", &query_str)
            && let Some(imports) = Self::collect_imports_with_compiled_query(root, content, &query)
        {
            return imports;
        }

        // Fallback: trait-based extraction via Language::extract_imports
        Self::collect_imports_with_trait(root, content, support)
    }

    /// Query-based import extraction using `@import`, `@import.path`, `@import.name`,
    /// `@import.alias`, and `@import.glob` captures.
    fn collect_imports_with_compiled_query(
        root: tree_sitter::Node,
        source: &str,
        query: &tree_sitter::Query,
    ) -> Option<Vec<FlatImport>> {
        let path_idx = query.capture_index_for_name("import.path");
        let name_idx = query.capture_index_for_name("import.name");
        let alias_idx = query.capture_index_for_name("import.alias");
        let glob_idx = query.capture_index_for_name("import.glob");
        let stmt_idx = query.capture_index_for_name("import");

        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut results = Vec::new();

        let mut matches = qcursor.matches(query, root, source.as_bytes());
        while let Some(m) = matches.next() {
            let mut stmt_line = 0usize;
            let mut path: Option<String> = None;
            let mut name: Option<String> = None;
            let mut alias: Option<String> = None;
            let mut is_glob = false;

            for cap in m.captures {
                let text = &source[cap.node.byte_range()];
                let idx = cap.index;
                if stmt_idx == Some(idx) {
                    stmt_line = cap.node.start_position().row + 1;
                } else if path_idx == Some(idx) {
                    path = Some(strip_import_quotes(text));
                } else if name_idx == Some(idx) {
                    name = Some(text.to_string());
                } else if alias_idx == Some(idx) {
                    alias = Some(text.to_string());
                } else if glob_idx == Some(idx) {
                    is_glob = true;
                }
            }

            if is_glob {
                results.push(FlatImport {
                    module: path,
                    name: "*".to_string(),
                    alias,
                    line: stmt_line,
                });
            } else if let Some(n) = name {
                results.push(FlatImport {
                    module: path,
                    name: n,
                    alias,
                    line: stmt_line,
                });
            } else if let Some(p) = path {
                results.push(FlatImport {
                    module: None,
                    name: p,
                    alias,
                    line: stmt_line,
                });
            }
        }
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }

    /// Trait-based import extraction fallback.
    /// Walks all nodes and calls `Language::extract_imports` on each.
    fn collect_imports_with_trait(
        root: tree_sitter::Node,
        source: &str,
        support: &dyn normalize_languages::Language,
    ) -> Vec<FlatImport> {
        let mut results = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            for import in support.extract_imports(&node, source) {
                if import.is_wildcard {
                    // Wildcard import: store name="*" with module
                    results.push(FlatImport {
                        module: Some(import.module.clone()),
                        name: "*".to_string(),
                        alias: None,
                        line: import.line,
                    });
                } else if import.names.is_empty() {
                    // Single import: module is the full path
                    results.push(FlatImport {
                        module: None,
                        name: import.module.clone(),
                        alias: import.alias.clone(),
                        line: import.line,
                    });
                } else {
                    // Named imports: one entry per name
                    for n in &import.names {
                        results.push(FlatImport {
                            module: Some(import.module.clone()),
                            name: n.clone(),
                            alias: import.alias.clone(),
                            line: import.line,
                        });
                    }
                }
            }
            // Push children in reverse order for DFS
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
        results
    }

    /// Find a symbol by name in a file
    pub fn find_symbol(&mut self, path: &Path, content: &str, name: &str) -> Option<FlatSymbol> {
        let symbols = self.parse_file(path, content);
        symbols.into_iter().find(|s| s.name == name)
    }

    /// Extract the source code for a symbol
    pub fn extract_symbol_source(
        &mut self,
        path: &Path,
        content: &str,
        name: &str,
    ) -> Option<String> {
        let symbol = self.find_symbol(path, content, name)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = symbol.start_line.saturating_sub(1);
        let end = symbol.end_line.min(lines.len());
        Some(lines[start..end].join("\n"))
    }

    /// Find callees (functions/methods called) within a symbol
    #[allow(dead_code)] // Call graph API - used by index
    pub fn find_callees(&mut self, path: &Path, content: &str, symbol_name: &str) -> Vec<String> {
        let symbol = match self.find_symbol(path, content, symbol_name) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let calls = self.find_callees_for_symbol(path, content, &symbol);
        let mut unique: std::collections::HashSet<String> =
            calls.into_iter().map(|(name, _, _, _)| name).collect();
        let mut result: Vec<_> = unique.drain().collect();
        result.sort();
        result
    }

    /// Find callees with line numbers (for call graph indexing)
    /// Returns: (callee_name, line, Option<qualifier>)
    /// For foo.bar(), returns ("bar", line, Some("foo"), access)
    /// For bar(), returns ("bar", line, None, access)
    /// `access` is `Some("write")` when the call result is assigned; `None` otherwise.
    #[allow(dead_code)] // Call graph API - used by index
    pub fn find_callees_with_lines(
        &mut self,
        path: &Path,
        content: &str,
        symbol_name: &str,
    ) -> Vec<(String, usize, Option<String>, Option<String>)> {
        let symbol = match self.find_symbol(path, content, symbol_name) {
            Some(s) => s,
            None => return Vec::new(),
        };
        self.find_callees_for_symbol(path, content, &symbol)
    }

    /// Find callees for a pre-parsed symbol (avoids re-parsing the file)
    /// Use this when you already have the FlatSymbol from parse_file()
    /// Returns `(callee_name, line, qualifier, access)` where `access` is
    /// `Some("write")` when the call result is assigned, `None` otherwise.
    pub fn find_callees_for_symbol(
        &mut self,
        path: &Path,
        content: &str,
        symbol: &FlatSymbol,
    ) -> Vec<(String, usize, Option<String>, Option<String>)> {
        let support = match support_for_path(path) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let grammar_name = support.grammar_name();
        let loader = normalize_languages::parsers::grammar_loader();

        let calls_query = match loader.get_calls(grammar_name) {
            Some(scm) => scm,
            None => return Vec::new(),
        };

        let query = match loader.get_compiled_query(grammar_name, "calls", &calls_query) {
            Some(q) => q,
            None => return Vec::new(),
        };

        let lines: Vec<&str> = content.lines().collect();
        let start = symbol.start_line.saturating_sub(1);
        let end = symbol.end_line.min(lines.len());
        let source = lines[start..end].join("\n");

        let tree = match parsers::parse_with_grammar(grammar_name, &source) {
            Some(t) => t,
            None => return Vec::new(),
        };

        Self::collect_calls_with_query(&tree.root_node(), &source, &query, symbol.start_line)
    }

    /// Generic query-based call extraction using `@call`, `@call.write`, and
    /// `@call.qualifier` captures.
    ///
    /// - `@call` — call in read context (access = None)
    /// - `@call.write` — call whose result is assigned (access = Some("write"))
    /// - `@call.qualifier` — qualifier/receiver for method calls
    ///
    /// When both `@call` and `@call.write` match the same node (same byte offset +
    /// line), the "write" tag wins.  This happens because write-context patterns use
    /// `@call.write` while the generic patterns still use `@call`; deduplication via
    /// a HashMap ensures a single entry per (name, line) pair with the most specific
    /// access tag.
    fn collect_calls_with_query(
        root: &tree_sitter::Node,
        source: &str,
        query: &tree_sitter::Query,
        base_line: usize,
    ) -> Vec<(String, usize, Option<String>, Option<String>)> {
        let call_idx = query.capture_names().iter().position(|n| *n == "call");
        let call_write_idx = query
            .capture_names()
            .iter()
            .position(|n| *n == "call.write");
        let qualifier_idx = query
            .capture_names()
            .iter()
            .position(|n| *n == "call.qualifier");

        if call_idx.is_none() && call_write_idx.is_none() {
            return Vec::new();
        }

        let mut qcursor = tree_sitter::QueryCursor::new();
        // Map (name, line) -> (qualifier, access) — "write" beats None
        let mut call_map: std::collections::HashMap<
            (String, usize),
            (Option<String>, Option<String>),
        > = std::collections::HashMap::new();

        let mut matches = qcursor.matches(query, *root, source.as_bytes());
        while let Some(m) = matches.next() {
            let mut name: Option<(&str, usize)> = None;
            let mut qualifier: Option<&str> = None;
            let mut is_write = false;

            for capture in m.captures {
                let idx = capture.index as usize;
                if Some(idx) == call_idx {
                    let text = &source[capture.node.byte_range()];
                    let line = capture.node.start_position().row + base_line;
                    name = Some((text, line));
                } else if Some(idx) == call_write_idx {
                    let text = &source[capture.node.byte_range()];
                    let line = capture.node.start_position().row + base_line;
                    name = Some((text, line));
                    is_write = true;
                } else if Some(idx) == qualifier_idx {
                    qualifier = Some(&source[capture.node.byte_range()]);
                }
            }

            if let Some((call_name, line)) = name {
                let access = if is_write {
                    Some("write".to_string())
                } else {
                    None
                };
                let key = (call_name.to_string(), line);
                let entry = call_map
                    .entry(key)
                    .or_insert((qualifier.map(|q| q.to_string()), None));
                // "write" wins over None
                if access.is_some() {
                    entry.1 = access;
                }
                // Update qualifier if present in this match
                if let Some(q) = qualifier {
                    entry.0 = Some(q.to_string());
                }
            }
        }

        call_map
            .into_iter()
            .map(|((name, line), (qualifier, access))| (name, line, qualifier, access))
            .collect()
    }

    /// Extract type references from a source file.
    /// Returns references to types found in struct fields, function params/returns,
    /// inheritance, trait bounds, type aliases, etc.
    pub fn find_type_refs(&mut self, path: &Path, content: &str) -> Vec<TypeRef> {
        let lang = match normalize_languages::support_for_path(path) {
            Some(l) => l,
            None => return Vec::new(),
        };
        match lang.name() {
            "Rust" => Self::find_rust_type_refs(content),
            "TypeScript" | "TSX" => Self::find_typescript_type_refs(content, lang.name() == "TSX"),
            "Python" => Self::find_python_type_refs(content),
            "Go" => Self::find_go_type_refs(content),
            "Java" => Self::find_java_type_refs(content),
            "C#" => Self::find_csharp_type_refs(content),
            "Kotlin" => Self::find_kotlin_type_refs(content),
            "Swift" => Self::find_swift_type_refs(content),
            "C++" => Self::find_cpp_type_refs(content),
            "Ruby" => Self::find_ruby_type_refs(content),
            _ => Vec::new(),
        }
    }

    /// Extract type references from Rust source code.
    fn find_rust_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("rust", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_rust_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_rust_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // struct Foo { field: BarType }
                "field_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    if let Some(type_node) = node.child_by_field_name("type") {
                        for type_name in Self::extract_type_identifiers(&type_node, content) {
                            refs.push(TypeRef {
                                source_symbol: container.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::FieldType,
                                line: type_node.start_position().row + 1,
                            });
                        }
                    }
                }
                // fn foo(x: BarType) -> BazType
                "function_item" | "function_signature_item" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_rust_param_types(&params, content, &fn_name, refs);
                    }
                    if let Some(ret) = node.child_by_field_name("return_type") {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            refs.push(TypeRef {
                                source_symbol: fn_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::ReturnType,
                                line: ret.start_position().row + 1,
                            });
                        }
                    }
                }
                // impl Trait for Type / impl Type
                "impl_item" => {
                    // Get the type being implemented
                    let impl_type = node
                        .child_by_field_name("type")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // Check for trait
                    if let Some(trait_node) = node.child_by_field_name("trait") {
                        for type_name in Self::extract_type_identifiers(&trait_node, content) {
                            refs.push(TypeRef {
                                source_symbol: impl_type.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::Implements,
                                line: trait_node.start_position().row + 1,
                            });
                        }
                    }
                }
                // trait Foo: Bar + Baz (supertraits)
                "trait_item" => {
                    let trait_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(bounds) = node.child_by_field_name("bounds") {
                        for type_name in Self::extract_type_identifiers(&bounds, content) {
                            refs.push(TypeRef {
                                source_symbol: trait_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::Extends,
                                line: bounds.start_position().row + 1,
                            });
                        }
                    }
                }
                // type Foo = Bar
                "type_item" => {
                    let alias_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(value) = node.child_by_field_name("type") {
                        for type_name in Self::extract_type_identifiers(&value, content) {
                            refs.push(TypeRef {
                                source_symbol: alias_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::TypeAlias,
                                line: value.start_position().row + 1,
                            });
                        }
                    }
                }
                // where T: Foo + Bar (type_bound_list in where clauses)
                "where_clause" => {
                    let fn_name = Self::ancestor_name(&node, content);
                    Self::collect_rust_where_bounds(&node, content, &fn_name, refs);
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_rust_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract parameter types from a Rust function's parameter list.
    fn collect_rust_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if (child.kind() == "parameter" || child.kind() == "self_parameter")
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    for type_name in Self::extract_type_identifiers(&type_node, content) {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::ParamType,
                            line: type_node.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type bounds from a Rust where clause.
    fn collect_rust_where_bounds(
        where_node: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = where_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "where_predicate"
                    && let Some(bounds) = child.child_by_field_name("bounds")
                {
                    for type_name in Self::extract_type_identifiers(&bounds, content) {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::GenericBound,
                            line: bounds.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type references from TypeScript source code.
    fn find_typescript_type_refs(content: &str, is_tsx: bool) -> Vec<TypeRef> {
        let grammar = if is_tsx { "tsx" } else { "typescript" };
        let tree = match parsers::parse_with_grammar(grammar, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_typescript_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_typescript_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo extends Bar implements Baz
                "class_declaration" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // Find extends and implements clauses
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            match child.kind() {
                                "extends_clause" => {
                                    for type_name in Self::extract_type_identifiers(&child, content)
                                    {
                                        refs.push(TypeRef {
                                            source_symbol: class_name.clone(),
                                            target_type: type_name,
                                            kind: TypeRefKind::Extends,
                                            line: child.start_position().row + 1,
                                        });
                                    }
                                }
                                "implements_clause" => {
                                    for type_name in Self::extract_type_identifiers(&child, content)
                                    {
                                        refs.push(TypeRef {
                                            source_symbol: class_name.clone(),
                                            target_type: type_name,
                                            kind: TypeRefKind::Implements,
                                            line: child.start_position().row + 1,
                                        });
                                    }
                                }
                                _ => {}
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // interface Foo extends Bar
                "interface_declaration" => {
                    let iface_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "extends_type_clause"
                                || child.kind() == "extends_clause"
                            {
                                for type_name in Self::extract_type_identifiers(&child, content) {
                                    refs.push(TypeRef {
                                        source_symbol: iface_name.clone(),
                                        target_type: type_name,
                                        kind: TypeRefKind::Extends,
                                        line: child.start_position().row + 1,
                                    });
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // type Foo = Bar
                "type_alias_declaration" => {
                    let alias_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(value) = node.child_by_field_name("value") {
                        for type_name in Self::extract_type_identifiers(&value, content) {
                            refs.push(TypeRef {
                                source_symbol: alias_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::TypeAlias,
                                line: value.start_position().row + 1,
                            });
                        }
                    }
                }
                // function foo(x: Bar): Baz  / method_definition / arrow functions
                "function_declaration" | "method_definition" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_ts_param_types(&params, content, &fn_name, refs);
                    }
                    if let Some(ret) = node.child_by_field_name("return_type") {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            refs.push(TypeRef {
                                source_symbol: fn_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::ReturnType,
                                line: ret.start_position().row + 1,
                            });
                        }
                    }
                }
                // Property with type annotation in interface/class
                "public_field_definition" | "property_signature" => {
                    let container = Self::ancestor_name(&node, content);
                    if let Some(type_ann) = node.child_by_field_name("type") {
                        for type_name in Self::extract_type_identifiers(&type_ann, content) {
                            refs.push(TypeRef {
                                source_symbol: container.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::FieldType,
                                line: type_ann.start_position().row + 1,
                            });
                        }
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_typescript_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract parameter types from TypeScript function parameters.
    fn collect_ts_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                // required_parameter, optional_parameter
                if child.kind().contains("parameter")
                    && let Some(type_ann) = child.child_by_field_name("type")
                {
                    for type_name in Self::extract_type_identifiers(&type_ann, content) {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::ParamType,
                            line: type_ann.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type references from Go source code.
    fn find_go_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("go", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_go_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_go_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // type Foo struct { Bar Baz }
                // type MyInterface interface { OtherInterface }
                // type Alias = Original
                "type_declaration" => {
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            match child.kind() {
                                "type_spec" => {
                                    Self::collect_go_type_spec(&child, content, refs);
                                }
                                "type_alias" => {
                                    Self::collect_go_type_alias(&child, content, refs);
                                }
                                _ => {}
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    // Don't recurse into type_declaration children below
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                    continue;
                }
                // func (r *Recv) Method(x Bar) Baz
                "method_declaration" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // Receiver type → field_type edge from recv_type → fn_name (skip, not typical)
                    // Params
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_go_param_types(&params, content, &fn_name, refs);
                    }
                    // Return type(s)
                    if let Some(result) = node.child_by_field_name("result") {
                        Self::collect_go_result_types(&result, content, &fn_name, refs);
                    }
                }
                // func Foo(x Bar) Baz
                "function_declaration" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_go_param_types(&params, content, &fn_name, refs);
                    }
                    if let Some(result) = node.child_by_field_name("result") {
                        Self::collect_go_result_types(&result, content, &fn_name, refs);
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_go_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Collect type refs from a Go `type_spec` node (struct or interface).
    fn collect_go_type_spec(node: &tree_sitter::Node, content: &str, refs: &mut Vec<TypeRef>) {
        let type_name = node
            .child_by_field_name("name")
            .map(|n| content[n.byte_range()].to_string())
            .unwrap_or_default();
        if type_name.is_empty() {
            return;
        }

        let type_body = match node.child_by_field_name("type") {
            Some(t) => t,
            None => return,
        };

        match type_body.kind() {
            // struct fields
            "struct_type" => {
                let mut cur = type_body.walk();
                if cur.goto_first_child() {
                    loop {
                        let child = cur.node();
                        if child.kind() == "field_declaration_list" {
                            let mut fc = child.walk();
                            if fc.goto_first_child() {
                                loop {
                                    let field = fc.node();
                                    if field.kind() == "field_declaration"
                                        && let Some(ft) = field.child_by_field_name("type")
                                    {
                                        // qualified_type (io.Reader) or type_identifier
                                        let type_name_str = Self::go_type_name(&ft, content);
                                        if !type_name_str.is_empty()
                                            && !Self::is_primitive_type(&type_name_str)
                                            && !Self::is_go_primitive(&type_name_str)
                                        {
                                            refs.push(TypeRef {
                                                source_symbol: type_name.clone(),
                                                target_type: type_name_str,
                                                kind: TypeRefKind::FieldType,
                                                line: ft.start_position().row + 1,
                                            });
                                        }
                                    }
                                    if !fc.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                        }
                        if !cur.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            // interface embedded types
            "interface_type" => {
                let mut cur = type_body.walk();
                if cur.goto_first_child() {
                    loop {
                        let child = cur.node();
                        // type_elem = embedded interface constraint
                        if child.kind() == "type_elem" {
                            let mut ec = child.walk();
                            if ec.goto_first_child() {
                                loop {
                                    let elem = ec.node();
                                    if elem.kind() == "type_identifier" {
                                        let embedded = content[elem.byte_range()].to_string();
                                        if !Self::is_primitive_type(&embedded)
                                            && !Self::is_go_primitive(&embedded)
                                        {
                                            refs.push(TypeRef {
                                                source_symbol: type_name.clone(),
                                                target_type: embedded,
                                                kind: TypeRefKind::Implements,
                                                line: elem.start_position().row + 1,
                                            });
                                        }
                                    }
                                    if !ec.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                        }
                        if !cur.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Collect type refs from a Go `type_alias` node (`type Alias = Original`).
    fn collect_go_type_alias(node: &tree_sitter::Node, content: &str, refs: &mut Vec<TypeRef>) {
        let alias_name = node
            .child_by_field_name("name")
            .map(|n| content[n.byte_range()].to_string())
            .unwrap_or_default();
        if alias_name.is_empty() {
            return;
        }
        if let Some(type_node) = node.child_by_field_name("type") {
            let target = Self::go_type_name(&type_node, content);
            if !target.is_empty()
                && !Self::is_primitive_type(&target)
                && !Self::is_go_primitive(&target)
            {
                refs.push(TypeRef {
                    source_symbol: alias_name,
                    target_type: target,
                    kind: TypeRefKind::TypeAlias,
                    line: type_node.start_position().row + 1,
                });
            }
        }
    }

    /// Extract a readable type name from a Go type node.
    /// For `qualified_type` (io.Reader), returns just the name part.
    /// For `type_identifier`, returns the identifier directly.
    fn go_type_name(node: &tree_sitter::Node, content: &str) -> String {
        match node.kind() {
            "type_identifier" => content[node.byte_range()].to_string(),
            "qualified_type" => {
                // package.Name — return just Name
                node.child_by_field_name("name")
                    .map(|n| content[n.byte_range()].to_string())
                    .unwrap_or_default()
            }
            "pointer_type" => {
                // *Foo — look through the pointer
                let mut c = node.walk();
                if c.goto_first_child() {
                    loop {
                        let child = c.node();
                        if child.kind() == "type_identifier" || child.kind() == "qualified_type" {
                            return Self::go_type_name(&child, content);
                        }
                        if !c.goto_next_sibling() {
                            break;
                        }
                    }
                }
                String::new()
            }
            "slice_type" | "array_type" => {
                // []Foo or [N]Foo — look at element type
                node.child_by_field_name("element")
                    .map(|n| Self::go_type_name(&n, content))
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Extract parameter types from a Go parameter list.
    fn collect_go_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter_declaration"
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    let type_name = Self::go_type_name(&type_node, content);
                    if !type_name.is_empty()
                        && !Self::is_primitive_type(&type_name)
                        && !Self::is_go_primitive(&type_name)
                    {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::ParamType,
                            line: type_node.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract return types from a Go result field (single type or parameter_list).
    fn collect_go_result_types(
        result: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        match result.kind() {
            "type_identifier" | "qualified_type" | "pointer_type" | "slice_type" | "array_type" => {
                let type_name = Self::go_type_name(result, content);
                if !type_name.is_empty()
                    && !Self::is_primitive_type(&type_name)
                    && !Self::is_go_primitive(&type_name)
                {
                    refs.push(TypeRef {
                        source_symbol: fn_name.to_string(),
                        target_type: type_name,
                        kind: TypeRefKind::ReturnType,
                        line: result.start_position().row + 1,
                    });
                }
            }
            // Multiple return values: (Foo, Bar, error)
            "parameter_list" => {
                let mut cursor = result.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "parameter_declaration"
                            && let Some(type_node) = child.child_by_field_name("type")
                        {
                            let type_name = Self::go_type_name(&type_node, content);
                            if !type_name.is_empty()
                                && !Self::is_primitive_type(&type_name)
                                && !Self::is_go_primitive(&type_name)
                            {
                                refs.push(TypeRef {
                                    source_symbol: fn_name.to_string(),
                                    target_type: type_name,
                                    kind: TypeRefKind::ReturnType,
                                    line: type_node.start_position().row + 1,
                                });
                            }
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Go-specific primitive/builtin types to skip.
    fn is_go_primitive(name: &str) -> bool {
        matches!(
            name,
            "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "uint"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "uintptr"
                | "float32"
                | "float64"
                | "complex64"
                | "complex128"
                | "bool"
                | "string"
                | "byte"
                | "rune"
                | "error"
        )
    }

    /// Extract type references from Java source code.
    fn find_java_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("java", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_java_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_java_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo extends Bar implements Baz, Qux { ... }
                "class_declaration" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // extends
                    if let Some(superclass) = node.child_by_field_name("superclass") {
                        for type_name in Self::extract_type_identifiers(&superclass, content) {
                            refs.push(TypeRef {
                                source_symbol: class_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::Extends,
                                line: superclass.start_position().row + 1,
                            });
                        }
                    }
                    // implements
                    if let Some(interfaces) = node.child_by_field_name("interfaces") {
                        for type_name in Self::extract_type_identifiers(&interfaces, content) {
                            refs.push(TypeRef {
                                source_symbol: class_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::Implements,
                                line: interfaces.start_position().row + 1,
                            });
                        }
                    }
                }
                // interface MyInterface extends OtherInterface { ... }
                "interface_declaration" => {
                    let iface_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // extends_interfaces child (not a named field)
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "extends_interfaces" {
                                for type_name in Self::extract_type_identifiers(&child, content) {
                                    refs.push(TypeRef {
                                        source_symbol: iface_name.clone(),
                                        target_type: type_name,
                                        kind: TypeRefKind::Extends,
                                        line: child.start_position().row + 1,
                                    });
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // private Bar field;
                "field_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    if let Some(type_node) = node.child_by_field_name("type") {
                        for type_name in Self::extract_type_identifiers(&type_node, content) {
                            refs.push(TypeRef {
                                source_symbol: container.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::FieldType,
                                line: type_node.start_position().row + 1,
                            });
                        }
                    }
                }
                // public Bar method(Baz param) { ... }
                "method_declaration" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // Return type
                    if let Some(ret) = node.child_by_field_name("type") {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            refs.push(TypeRef {
                                source_symbol: fn_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::ReturnType,
                                line: ret.start_position().row + 1,
                            });
                        }
                    }
                    // Parameters
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_java_param_types(&params, content, &fn_name, refs);
                    }
                    // Generic bounds: <T extends Bound>
                    if let Some(type_params) = node.child_by_field_name("type_parameters") {
                        Self::collect_java_generic_bounds(&type_params, content, &fn_name, refs);
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_java_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract parameter types from Java formal_parameters.
    fn collect_java_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "formal_parameter"
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    for type_name in Self::extract_type_identifiers(&type_node, content) {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::ParamType,
                            line: type_node.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract generic bounds from Java type_parameters (<T extends Bound>).
    fn collect_java_generic_bounds(
        type_params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = type_params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_parameter" {
                    // type_bound child: extends SomeType
                    let mut tc = child.walk();
                    if tc.goto_first_child() {
                        loop {
                            let tc_child = tc.node();
                            if tc_child.kind() == "type_bound" {
                                for type_name in Self::extract_type_identifiers(&tc_child, content)
                                {
                                    refs.push(TypeRef {
                                        source_symbol: fn_name.to_string(),
                                        target_type: type_name,
                                        kind: TypeRefKind::GenericBound,
                                        line: tc_child.start_position().row + 1,
                                    });
                                }
                            }
                            if !tc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type references from Python source code.
    fn find_python_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("python", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_python_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_python_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo(Bar, Baz):
                "class_definition" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(bases) = node.child_by_field_name("superclasses") {
                        // argument_list containing identifiers
                        let mut base_cursor = bases.walk();
                        if base_cursor.goto_first_child() {
                            loop {
                                let base = base_cursor.node();
                                if base.kind() == "identifier" || base.kind() == "attribute" {
                                    let base_name = content[base.byte_range()].to_string();
                                    refs.push(TypeRef {
                                        source_symbol: class_name.clone(),
                                        target_type: base_name,
                                        kind: TypeRefKind::Extends,
                                        line: base.start_position().row + 1,
                                    });
                                }
                                if !base_cursor.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                }
                // def foo(x: Bar) -> Baz:
                "function_definition" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_python_param_types(&params, content, &fn_name, refs);
                    }
                    if let Some(ret) = node.child_by_field_name("return_type") {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            refs.push(TypeRef {
                                source_symbol: fn_name.clone(),
                                target_type: type_name,
                                kind: TypeRefKind::ReturnType,
                                line: ret.start_position().row + 1,
                            });
                        }
                    }
                }
                // x: int = 5 (variable type annotations at class level)
                "typed_parameter" | "typed_default_parameter" => {
                    // Handled in collect_python_param_types
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_python_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Extract parameter types from Python function parameters.
    fn collect_python_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                // typed_parameter: name: type, typed_default_parameter: name: type = default
                if (child.kind() == "typed_parameter" || child.kind() == "typed_default_parameter")
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    for type_name in Self::extract_type_identifiers(&type_node, content) {
                        // Skip 'self' parameter type
                        if type_name != "self" {
                            refs.push(TypeRef {
                                source_symbol: fn_name.to_string(),
                                target_type: type_name,
                                kind: TypeRefKind::ParamType,
                                line: type_node.start_position().row + 1,
                            });
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // --- C# ---

    /// Extract type references from C# source code.
    fn find_csharp_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("c-sharp", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_csharp_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_csharp_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo : Bar, IBaz { ... }
                "class_declaration"
                | "interface_declaration"
                | "struct_declaration"
                | "record_declaration" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // base_list contains comma-separated base types
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "base_list" {
                                let mut bl = child.walk();
                                if bl.goto_first_child() {
                                    loop {
                                        let base = bl.node();
                                        // Each entry is identifier, generic_name, or qualified_name
                                        let type_name = match base.kind() {
                                            "identifier" | "qualified_name" => {
                                                content[base.byte_range()].to_string()
                                            }
                                            "generic_name" => base
                                                .child_by_field_name("name")
                                                .map(|n| content[n.byte_range()].to_string())
                                                .unwrap_or_default(),
                                            _ => String::new(),
                                        };
                                        if !type_name.is_empty()
                                            && !Self::is_primitive_type(&type_name)
                                            && !Self::is_csharp_primitive(&type_name)
                                        {
                                            // Heuristic: interface names conventionally start with I
                                            // but we emit Implements for all base types since C# has
                                            // both extends and implements via base_list
                                            let ref_kind = if kind == "interface_declaration" {
                                                TypeRefKind::Extends
                                            } else {
                                                TypeRefKind::Implements
                                            };
                                            refs.push(TypeRef {
                                                source_symbol: class_name.clone(),
                                                target_type: type_name,
                                                kind: ref_kind,
                                                line: base.start_position().row + 1,
                                            });
                                        }
                                        if !bl.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // private Bar _field;
                // C# grammar: field_declaration > variable_declaration > identifier (type) + variable_declarator
                "field_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    let mut fc = node.walk();
                    if fc.goto_first_child() {
                        loop {
                            let child = fc.node();
                            if child.kind() == "variable_declaration" {
                                // First identifier child is the type
                                let mut vc = child.walk();
                                if vc.goto_first_child() {
                                    let type_node = vc.node();
                                    if type_node.kind() == "identifier"
                                        || type_node.kind() == "generic_name"
                                        || type_node.kind() == "qualified_name"
                                        || type_node.kind() == "nullable_type"
                                    {
                                        for type_name in
                                            Self::extract_type_identifiers(&type_node, content)
                                        {
                                            if !Self::is_csharp_primitive(&type_name) {
                                                refs.push(TypeRef {
                                                    source_symbol: container.clone(),
                                                    target_type: type_name,
                                                    kind: TypeRefKind::FieldType,
                                                    line: type_node.start_position().row + 1,
                                                });
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                            if !fc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // public Response Method(Request param) { ... }
                // C# grammar: method_declaration has identifier children in order:
                //   [modifier] [return_type_identifier] [method_name_identifier] [parameter_list]
                // There are no named "type" or "name" fields — positional.
                "method_declaration" => {
                    // Collect identifiers in order; first is return type, second is name
                    let mut identifiers: Vec<tree_sitter::Node> = Vec::new();
                    let mut param_list: Option<tree_sitter::Node> = None;
                    let mut mc = node.walk();
                    if mc.goto_first_child() {
                        loop {
                            let child = mc.node();
                            match child.kind() {
                                "identifier" | "generic_name" | "qualified_name"
                                | "nullable_type" | "predefined_type" => {
                                    identifiers.push(child);
                                }
                                "parameter_list" => {
                                    param_list = Some(child);
                                }
                                _ => {}
                            }
                            if !mc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    // identifiers[0] = return type, identifiers[1] = method name
                    let fn_name = identifiers
                        .get(1)
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if let Some(ret_node) = identifiers.first() {
                        for type_name in Self::extract_type_identifiers(ret_node, content) {
                            if !Self::is_csharp_primitive(&type_name) {
                                refs.push(TypeRef {
                                    source_symbol: fn_name.clone(),
                                    target_type: type_name,
                                    kind: TypeRefKind::ReturnType,
                                    line: ret_node.start_position().row + 1,
                                });
                            }
                        }
                    }
                    // Parameters
                    if let Some(params) = param_list {
                        Self::collect_csharp_param_types(&params, content, &fn_name, refs);
                    }
                    // Generic type constraints
                    if !fn_name.is_empty() {
                        Self::collect_csharp_generic_bounds(&node, content, &fn_name, refs);
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_csharp_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn collect_csharp_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter" {
                    // C# parameter: first identifier/generic_name = type, second = name
                    let mut pc = child.walk();
                    if pc.goto_first_child() {
                        let type_node = pc.node();
                        if matches!(
                            type_node.kind(),
                            "identifier"
                                | "generic_name"
                                | "qualified_name"
                                | "nullable_type"
                                | "predefined_type"
                        ) {
                            for type_name in Self::extract_type_identifiers(&type_node, content) {
                                if !Self::is_csharp_primitive(&type_name) {
                                    refs.push(TypeRef {
                                        source_symbol: fn_name.to_string(),
                                        target_type: type_name,
                                        kind: TypeRefKind::ParamType,
                                        line: type_node.start_position().row + 1,
                                    });
                                }
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type constraints from a C# method's type_parameter_constraints_clauses.
    fn collect_csharp_generic_bounds(
        method_node: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = method_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_parameter_constraints_clause" {
                    // Contains: "where" TypeParam ":" constraint_list
                    let mut cc = child.walk();
                    if cc.goto_first_child() {
                        loop {
                            let cc_child = cc.node();
                            // type_constraint contains the bound type
                            if cc_child.kind() == "type_constraint"
                                || cc_child.kind() == "constructor_constraint"
                            {
                                for type_name in Self::extract_type_identifiers(&cc_child, content)
                                {
                                    if !Self::is_csharp_primitive(&type_name) {
                                        refs.push(TypeRef {
                                            source_symbol: fn_name.to_string(),
                                            target_type: type_name,
                                            kind: TypeRefKind::GenericBound,
                                            line: cc_child.start_position().row + 1,
                                        });
                                    }
                                }
                            }
                            if !cc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn is_csharp_primitive(name: &str) -> bool {
        matches!(
            name,
            "int"
                | "uint"
                | "long"
                | "ulong"
                | "short"
                | "ushort"
                | "byte"
                | "sbyte"
                | "float"
                | "double"
                | "decimal"
                | "bool"
                | "char"
                | "string"
                | "object"
                | "void"
                | "dynamic"
                | "var"
        )
    }

    // --- Kotlin ---

    /// Extract type references from Kotlin source code.
    fn find_kotlin_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("kotlin", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_kotlin_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_kotlin_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo : Bar(), IBaz { ... }
                "class_declaration" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_else(|| {
                            // Kotlin class_declaration uses first type_identifier when no "name" field
                            let mut cur = node.walk();
                            let mut name = String::new();
                            if cur.goto_first_child() {
                                loop {
                                    if cur.node().kind() == "type_identifier" {
                                        name = content[cur.node().byte_range()].to_string();
                                        break;
                                    }
                                    if !cur.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            name
                        });
                    // delegation_specifiers are direct children (not a named field)
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "delegation_specifier" {
                                // delegation_specifier contains a constructor_invocation or
                                // user_type — extract the type_identifier
                                let type_name = Self::kotlin_first_type_identifier(&child, content);
                                if !type_name.is_empty()
                                    && !Self::is_primitive_type(&type_name)
                                    && !Self::is_kotlin_primitive(&type_name)
                                {
                                    refs.push(TypeRef {
                                        source_symbol: class_name.clone(),
                                        target_type: type_name,
                                        kind: TypeRefKind::Implements,
                                        line: child.start_position().row + 1,
                                    });
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // val foo: Bar = ...
                // Kotlin grammar: property_declaration > variable_declaration > user_type > type_identifier
                "property_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    // Walk into variable_declaration to find the user_type
                    let mut pc = node.walk();
                    if pc.goto_first_child() {
                        loop {
                            let child = pc.node();
                            if child.kind() == "variable_declaration" {
                                // user_type is a child of variable_declaration
                                let mut vc = child.walk();
                                if vc.goto_first_child() {
                                    loop {
                                        let vc_child = vc.node();
                                        if vc_child.kind() == "user_type"
                                            || vc_child.kind() == "nullable_type"
                                            || vc_child.kind() == "type_identifier"
                                        {
                                            for type_name in
                                                Self::extract_type_identifiers(&vc_child, content)
                                            {
                                                if !Self::is_kotlin_primitive(&type_name) {
                                                    refs.push(TypeRef {
                                                        source_symbol: container.clone(),
                                                        target_type: type_name,
                                                        kind: TypeRefKind::FieldType,
                                                        line: vc_child.start_position().row + 1,
                                                    });
                                                }
                                            }
                                        }
                                        if !vc.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                                break;
                            }
                            if !pc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // fun foo(x: Bar): Baz { ... }
                // Kotlin grammar: function_declaration > simple_identifier (name) >
                //   function_value_parameters > parameter > user_type >
                //   : > user_type (return type)
                "function_declaration" => {
                    // Find the function name (first simple_identifier) and collect
                    // function_value_parameters and return type (user_type after params)
                    let mut fn_name = String::new();
                    let mut params_node: Option<tree_sitter::Node> = None;
                    let mut return_type_node: Option<tree_sitter::Node> = None;
                    let mut found_params = false;
                    let mut cur = node.walk();
                    if cur.goto_first_child() {
                        loop {
                            let child = cur.node();
                            match child.kind() {
                                "simple_identifier" if fn_name.is_empty() => {
                                    fn_name = content[child.byte_range()].to_string();
                                }
                                "function_value_parameters" => {
                                    params_node = Some(child);
                                    found_params = true;
                                }
                                // Return type comes after function_value_parameters as user_type
                                "user_type" | "nullable_type" | "type_identifier"
                                    if found_params =>
                                {
                                    return_type_node = Some(child);
                                }
                                _ => {}
                            }
                            if !cur.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    if let Some(ret) = return_type_node {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            if !Self::is_kotlin_primitive(&type_name) {
                                refs.push(TypeRef {
                                    source_symbol: fn_name.clone(),
                                    target_type: type_name,
                                    kind: TypeRefKind::ReturnType,
                                    line: ret.start_position().row + 1,
                                });
                            }
                        }
                    }
                    if let Some(params) = params_node {
                        Self::collect_kotlin_param_types(&params, content, &fn_name, refs);
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_kotlin_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn collect_kotlin_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        // Kotlin function_value_parameters contains parameter nodes directly
        // Each parameter: simple_identifier : user_type
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter" {
                    // Find user_type or nullable_type child (after simple_identifier and :)
                    let mut pc = child.walk();
                    if pc.goto_first_child() {
                        loop {
                            let param_child = pc.node();
                            if matches!(
                                param_child.kind(),
                                "user_type" | "nullable_type" | "type_identifier"
                            ) {
                                for type_name in
                                    Self::extract_type_identifiers(&param_child, content)
                                {
                                    if !Self::is_kotlin_primitive(&type_name) {
                                        refs.push(TypeRef {
                                            source_symbol: fn_name.to_string(),
                                            target_type: type_name,
                                            kind: TypeRefKind::ParamType,
                                            line: param_child.start_position().row + 1,
                                        });
                                    }
                                }
                            }
                            if !pc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Find the first type_identifier in a Kotlin delegation_specifier subtree.
    fn kotlin_first_type_identifier(node: &tree_sitter::Node, content: &str) -> String {
        if node.kind() == "type_identifier" {
            return content[node.byte_range()].to_string();
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let result = Self::kotlin_first_type_identifier(&cursor.node(), content);
                if !result.is_empty() {
                    return result;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        String::new()
    }

    fn is_kotlin_primitive(name: &str) -> bool {
        matches!(
            name,
            "Int"
                | "Long"
                | "Short"
                | "Byte"
                | "Float"
                | "Double"
                | "Boolean"
                | "Char"
                | "String"
                | "Unit"
                | "Nothing"
                | "Any"
                | "Number"
                | "Array"
                | "List"
                | "MutableList"
                | "Map"
                | "MutableMap"
                | "Set"
                | "MutableSet"
        )
    }

    // --- Swift ---

    /// Extract type references from Swift source code.
    fn find_swift_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("swift", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_swift_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_swift_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo: Bar, Proto { ... } or struct/enum/actor
                "class_declaration" | "struct_declaration" | "enum_declaration"
                | "actor_declaration" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // inheritance_specifier children contain the inherited types
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "inheritance_specifier" {
                                let type_name = Self::swift_first_type_identifier(&child, content);
                                if !type_name.is_empty() && !Self::is_swift_primitive(&type_name) {
                                    refs.push(TypeRef {
                                        source_symbol: class_name.clone(),
                                        target_type: type_name,
                                        kind: TypeRefKind::Implements,
                                        line: child.start_position().row + 1,
                                    });
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // protocol Foo: Bar { ... }
                "protocol_declaration" => {
                    let proto_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "inheritance_specifier" {
                                let type_name = Self::swift_first_type_identifier(&child, content);
                                if !type_name.is_empty() && !Self::is_swift_primitive(&type_name) {
                                    refs.push(TypeRef {
                                        source_symbol: proto_name.clone(),
                                        target_type: type_name,
                                        kind: TypeRefKind::Extends,
                                        line: child.start_position().row + 1,
                                    });
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // var foo: Bar = ...
                "property_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    // type_annotation child contains the type
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "type_annotation" {
                                for type_name in Self::extract_type_identifiers(&child, content) {
                                    if !Self::is_swift_primitive(&type_name) {
                                        refs.push(TypeRef {
                                            source_symbol: container.clone(),
                                            target_type: type_name,
                                            kind: TypeRefKind::FieldType,
                                            line: child.start_position().row + 1,
                                        });
                                    }
                                }
                                break;
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // func foo(req: Bar) -> Baz { ... }
                // Swift grammar: function_declaration has direct parameter children (not in a
                // parameters node). Parameters come between ( and ).
                // Return type is user_type after the -> token.
                "function_declaration" => {
                    let fn_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_else(|| {
                            let mut cur = node.walk();
                            let mut name = String::new();
                            if cur.goto_first_child() {
                                loop {
                                    if cur.node().kind() == "simple_identifier" {
                                        name = content[cur.node().byte_range()].to_string();
                                        break;
                                    }
                                    if !cur.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            name
                        });
                    // Return type: "return_type" field, or user_type after ->
                    if let Some(ret) = node.child_by_field_name("return_type") {
                        for type_name in Self::extract_type_identifiers(&ret, content) {
                            if !Self::is_swift_primitive(&type_name) {
                                refs.push(TypeRef {
                                    source_symbol: fn_name.clone(),
                                    target_type: type_name,
                                    kind: TypeRefKind::ReturnType,
                                    line: ret.start_position().row + 1,
                                });
                            }
                        }
                    } else {
                        // Find user_type that appears after the arrow (->)
                        let mut cur = node.walk();
                        let mut after_arrow = false;
                        if cur.goto_first_child() {
                            loop {
                                let child = cur.node();
                                if child.kind() == "->" {
                                    after_arrow = true;
                                } else if after_arrow
                                    && matches!(
                                        child.kind(),
                                        "user_type" | "optional_type" | "type_identifier"
                                    )
                                {
                                    for type_name in Self::extract_type_identifiers(&child, content)
                                    {
                                        if !Self::is_swift_primitive(&type_name) {
                                            refs.push(TypeRef {
                                                source_symbol: fn_name.clone(),
                                                target_type: type_name,
                                                kind: TypeRefKind::ReturnType,
                                                line: child.start_position().row + 1,
                                            });
                                        }
                                    }
                                    break;
                                }
                                if !cur.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                    // Parameters: collect parameter children of function_declaration
                    // (they are direct children, not in a parameters field)
                    if let Some(params) = node.child_by_field_name("parameters") {
                        Self::collect_swift_param_types(&params, content, &fn_name, refs);
                    } else {
                        // Parameters are direct children between ( and )
                        let mut cur = node.walk();
                        if cur.goto_first_child() {
                            loop {
                                let child = cur.node();
                                if child.kind() == "parameter" {
                                    Self::collect_swift_param_type_from_node(
                                        &child, content, &fn_name, refs,
                                    );
                                }
                                if !cur.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_swift_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn collect_swift_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter" {
                    Self::collect_swift_param_type_from_node(&child, content, fn_name, refs);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract param type from a single Swift `parameter` node.
    /// Swift parameter structure: simple_identifier : user_type
    fn collect_swift_param_type_from_node(
        param: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut pc = param.walk();
        if pc.goto_first_child() {
            loop {
                let param_child = pc.node();
                if matches!(
                    param_child.kind(),
                    "user_type" | "optional_type" | "type_identifier" | "type_annotation"
                ) {
                    for type_name in Self::extract_type_identifiers(&param_child, content) {
                        if !Self::is_swift_primitive(&type_name) {
                            refs.push(TypeRef {
                                source_symbol: fn_name.to_string(),
                                target_type: type_name,
                                kind: TypeRefKind::ParamType,
                                line: param_child.start_position().row + 1,
                            });
                        }
                    }
                    break;
                }
                if !pc.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Find the first type_identifier in a Swift inheritance_specifier subtree.
    fn swift_first_type_identifier(node: &tree_sitter::Node, content: &str) -> String {
        if node.kind() == "type_identifier" {
            return content[node.byte_range()].to_string();
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let result = Self::swift_first_type_identifier(&cursor.node(), content);
                if !result.is_empty() {
                    return result;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        String::new()
    }

    fn is_swift_primitive(name: &str) -> bool {
        matches!(
            name,
            "Int"
                | "Int8"
                | "Int16"
                | "Int32"
                | "Int64"
                | "UInt"
                | "UInt8"
                | "UInt16"
                | "UInt32"
                | "UInt64"
                | "Float"
                | "Double"
                | "Float16"
                | "Float80"
                | "Bool"
                | "Character"
                | "String"
                | "Void"
                | "Never"
                | "Any"
                | "AnyObject"
                | "Optional"
                | "Array"
                | "Dictionary"
                | "Set"
        )
    }

    // --- C++ ---

    /// Extract type references from C++ source code.
    fn find_cpp_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("cpp", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_cpp_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_cpp_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Derived : public Base { ... }
                "class_specifier" | "struct_specifier" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // base_class_clause contains the base types
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "base_class_clause" {
                                let mut bc = child.walk();
                                if bc.goto_first_child() {
                                    loop {
                                        let base = bc.node();
                                        let type_name = match base.kind() {
                                            "type_identifier" => {
                                                content[base.byte_range()].to_string()
                                            }
                                            "qualified_identifier" => base
                                                .child_by_field_name("name")
                                                .map(|n| content[n.byte_range()].to_string())
                                                .unwrap_or_default(),
                                            _ => String::new(),
                                        };
                                        if !type_name.is_empty()
                                            && !Self::is_primitive_type(&type_name)
                                            && !Self::is_cpp_primitive(&type_name)
                                        {
                                            refs.push(TypeRef {
                                                source_symbol: class_name.clone(),
                                                target_type: type_name,
                                                kind: TypeRefKind::Extends,
                                                line: base.start_position().row + 1,
                                            });
                                        }
                                        if !bc.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    // Recurse into body for field declarations
                    if cursor.goto_first_child() {
                        Self::collect_cpp_type_refs(cursor, content, refs);
                        cursor.goto_parent();
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                    continue;
                }
                // int field_name;  or  MyType field_name;
                "field_declaration" => {
                    let container = Self::ancestor_name(&node, content);
                    if let Some(type_node) = node.child_by_field_name("type") {
                        let type_name = Self::cpp_type_name(&type_node, content);
                        if !type_name.is_empty()
                            && !Self::is_primitive_type(&type_name)
                            && !Self::is_cpp_primitive(&type_name)
                        {
                            refs.push(TypeRef {
                                source_symbol: container,
                                target_type: type_name,
                                kind: TypeRefKind::FieldType,
                                line: type_node.start_position().row + 1,
                            });
                        }
                    }
                }
                // ReturnType funcName(Params) { ... }
                "function_definition" => {
                    // Get return type from "type" field
                    let fn_name = node
                        .child_by_field_name("declarator")
                        .and_then(|d| Self::cpp_function_name(&d, content))
                        .unwrap_or_default();
                    if !fn_name.is_empty() {
                        if let Some(ret_node) = node.child_by_field_name("type") {
                            let type_name = Self::cpp_type_name(&ret_node, content);
                            if !type_name.is_empty()
                                && !Self::is_primitive_type(&type_name)
                                && !Self::is_cpp_primitive(&type_name)
                            {
                                refs.push(TypeRef {
                                    source_symbol: fn_name.clone(),
                                    target_type: type_name,
                                    kind: TypeRefKind::ReturnType,
                                    line: ret_node.start_position().row + 1,
                                });
                            }
                        }
                        // Parameters: find parameter_list in declarator
                        if let Some(declarator) = node.child_by_field_name("declarator") {
                            Self::collect_cpp_param_types_from_declarator(
                                &declarator,
                                content,
                                &fn_name,
                                refs,
                            );
                        }
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_cpp_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn collect_cpp_param_types_from_declarator(
        declarator: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        // function_declarator has a "parameters" field
        if declarator.kind() == "function_declarator" {
            if let Some(params) = declarator.child_by_field_name("parameters") {
                Self::collect_cpp_param_types(&params, content, fn_name, refs);
            }
        } else {
            // recurse into nested declarators (pointer_declarator, etc.)
            let mut cursor = declarator.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "function_declarator" {
                        if let Some(params) = child.child_by_field_name("parameters") {
                            Self::collect_cpp_param_types(&params, content, fn_name, refs);
                        }
                        break;
                    }
                    // recurse one more level
                    Self::collect_cpp_param_types_from_declarator(&child, content, fn_name, refs);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    fn collect_cpp_param_types(
        params: &tree_sitter::Node,
        content: &str,
        fn_name: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter_declaration"
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    let type_name = Self::cpp_type_name(&type_node, content);
                    if !type_name.is_empty()
                        && !Self::is_primitive_type(&type_name)
                        && !Self::is_cpp_primitive(&type_name)
                    {
                        refs.push(TypeRef {
                            source_symbol: fn_name.to_string(),
                            target_type: type_name,
                            kind: TypeRefKind::ParamType,
                            line: type_node.start_position().row + 1,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a usable type name from a C++ type node.
    fn cpp_type_name(node: &tree_sitter::Node, content: &str) -> String {
        match node.kind() {
            "type_identifier" => content[node.byte_range()].to_string(),
            "qualified_identifier" => node
                .child_by_field_name("name")
                .map(|n| content[n.byte_range()].to_string())
                .unwrap_or_default(),
            "template_type" => node
                .child_by_field_name("name")
                .map(|n| content[n.byte_range()].to_string())
                .unwrap_or_default(),
            // pointer/reference type: recurse into the inner type
            "pointer_declarator"
            | "reference_declarator"
            | "abstract_pointer_declarator"
            | "abstract_reference_declarator" => {
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if matches!(
                            child.kind(),
                            "type_identifier" | "qualified_identifier" | "template_type"
                        ) {
                            return Self::cpp_type_name(&child, content);
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                String::new()
            }
            _ => String::new(),
        }
    }

    /// Extract a function name from a C++ declarator node.
    fn cpp_function_name(declarator: &tree_sitter::Node, content: &str) -> Option<String> {
        match declarator.kind() {
            "function_declarator" => declarator
                .child_by_field_name("declarator")
                .and_then(|d| Self::cpp_function_name(&d, content)),
            "identifier" | "field_identifier" => Some(content[declarator.byte_range()].to_string()),
            "qualified_identifier" => declarator
                .child_by_field_name("name")
                .map(|n| content[n.byte_range()].to_string()),
            "pointer_declarator" | "reference_declarator" => {
                let mut cursor = declarator.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if let Some(name) = Self::cpp_function_name(&child, content) {
                            return Some(name);
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn is_cpp_primitive(name: &str) -> bool {
        matches!(
            name,
            "int"
                | "long"
                | "short"
                | "char"
                | "float"
                | "double"
                | "void"
                | "bool"
                | "auto"
                | "size_t"
                | "ptrdiff_t"
                | "nullptr_t"
                | "wchar_t"
                | "char8_t"
                | "char16_t"
                | "char32_t"
        )
    }

    // --- Ruby ---

    /// Extract type references from Ruby source code.
    fn find_ruby_type_refs(content: &str) -> Vec<TypeRef> {
        let tree = match parsers::parse_with_grammar("ruby", content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut refs = Vec::new();
        let mut cursor = tree.root_node().walk();
        Self::collect_ruby_type_refs(&mut cursor, content, &mut refs);
        refs
    }

    fn collect_ruby_type_refs(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        refs: &mut Vec<TypeRef>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            match kind {
                // class Foo < Bar
                "class" => {
                    let class_name = node
                        .child_by_field_name("name")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    // "superclass" child node
                    let mut child_cursor = node.walk();
                    if child_cursor.goto_first_child() {
                        loop {
                            let child = child_cursor.node();
                            if child.kind() == "superclass" {
                                // superclass contains a constant or scope_resolution
                                let mut sc = child.walk();
                                if sc.goto_first_child() {
                                    loop {
                                        let sc_child = sc.node();
                                        let type_name = match sc_child.kind() {
                                            "constant" => {
                                                content[sc_child.byte_range()].to_string()
                                            }
                                            "scope_resolution" => sc_child
                                                .child_by_field_name("name")
                                                .map(|n| content[n.byte_range()].to_string())
                                                .unwrap_or_default(),
                                            _ => String::new(),
                                        };
                                        if !type_name.is_empty() {
                                            refs.push(TypeRef {
                                                source_symbol: class_name.clone(),
                                                target_type: type_name,
                                                kind: TypeRefKind::Extends,
                                                line: sc_child.start_position().row + 1,
                                            });
                                        }
                                        if !sc.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                                break;
                            }
                            if !child_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // include Foo or prepend Bar
                "call" => {
                    // Check if this is a bare include/prepend call
                    let method_name = node
                        .child_by_field_name("method")
                        .map(|n| content[n.byte_range()].to_string())
                        .unwrap_or_default();
                    if method_name == "include" || method_name == "prepend" {
                        let container = Self::ancestor_name(&node, content);
                        if let Some(args) = node.child_by_field_name("arguments") {
                            let mut arg_cursor = args.walk();
                            if arg_cursor.goto_first_child() {
                                loop {
                                    let arg = arg_cursor.node();
                                    let type_name = match arg.kind() {
                                        "constant" => content[arg.byte_range()].to_string(),
                                        "scope_resolution" => arg
                                            .child_by_field_name("name")
                                            .map(|n| content[n.byte_range()].to_string())
                                            .unwrap_or_default(),
                                        _ => String::new(),
                                    };
                                    if !type_name.is_empty() {
                                        refs.push(TypeRef {
                                            source_symbol: container.clone(),
                                            target_type: type_name,
                                            kind: TypeRefKind::Implements,
                                            line: arg.start_position().row + 1,
                                        });
                                    }
                                    if !arg_cursor.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                Self::collect_ruby_type_refs(cursor, content, refs);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    // --- Helpers ---

    /// Walk up the AST to find the nearest named ancestor (struct, impl, trait, class, function).
    fn ancestor_name(node: &tree_sitter::Node, content: &str) -> String {
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "struct_item"
                | "enum_item"
                | "impl_item"
                | "trait_item"
                | "function_item"
                // Java/C#/Kotlin/Swift
                | "class_declaration"
                | "interface_declaration"
                | "struct_declaration"
                | "record_declaration"
                | "protocol_declaration"
                | "actor_declaration"
                | "method_declaration"
                // Python
                | "class_definition"
                | "function_definition"
                // C++
                | "class_specifier"
                | "struct_specifier"
                // Ruby
                | "class"
                | "module" => {
                    if let Some(name_node) = parent.child_by_field_name("name") {
                        return content[name_node.byte_range()].to_string();
                    }
                    // impl_item uses "type" field for the implemented type
                    if parent.kind() == "impl_item"
                        && let Some(type_node) = parent.child_by_field_name("type")
                    {
                        return content[type_node.byte_range()].to_string();
                    }
                    // C++ class_specifier/struct_specifier: name is a direct child type_identifier
                    if matches!(parent.kind(), "class_specifier" | "struct_specifier") {
                        let mut cur = parent.walk();
                        if cur.goto_first_child() {
                            loop {
                                if cur.node().kind() == "type_identifier" {
                                    return content[cur.node().byte_range()].to_string();
                                }
                                if !cur.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                    // Kotlin class_declaration: name may be first type_identifier
                    if parent.kind() == "class_declaration" {
                        let mut cur = parent.walk();
                        if cur.goto_first_child() {
                            loop {
                                let k = cur.node().kind();
                                if k == "type_identifier" || k == "simple_identifier" {
                                    return content[cur.node().byte_range()].to_string();
                                }
                                if !cur.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            current = parent.parent();
        }
        "<module>".to_string()
    }

    /// Extract all type_identifier nodes from a type expression.
    /// Handles generics (Vec<Foo>), references (&Foo), tuples, etc.
    /// Filters out primitive/builtin types.
    fn extract_type_identifiers(node: &tree_sitter::Node, content: &str) -> Vec<String> {
        let mut types = Vec::new();
        Self::collect_type_identifiers_recursive(node, content, &mut types);
        types
    }

    fn collect_type_identifiers_recursive(
        node: &tree_sitter::Node,
        content: &str,
        types: &mut Vec<String>,
    ) {
        let kind = node.kind();

        // Rust: type_identifier, TypeScript: type_identifier, Python: identifier/attribute
        if kind == "type_identifier" || kind == "identifier" {
            let name = content[node.byte_range()].to_string();
            if !Self::is_primitive_type(&name) {
                types.push(name);
            }
            return;
        }

        // Scoped type: path::to::Type — take the last segment
        if kind == "scoped_type_identifier" || kind == "scoped_identifier" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = content[name_node.byte_range()].to_string();
                if !Self::is_primitive_type(&name) {
                    types.push(name);
                }
            }
            return;
        }

        // Python attribute access: module.Type
        if kind == "attribute" {
            let text = content[node.byte_range()].to_string();
            if let Some(last) = text.rsplit('.').next()
                && !Self::is_primitive_type(last)
            {
                types.push(last.to_string());
            }
            return;
        }

        // Recurse into children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_type_identifiers_recursive(&cursor.node(), content, types);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Check if a type name is a primitive/builtin that we shouldn't track.
    fn is_primitive_type(name: &str) -> bool {
        matches!(
            name,
            // Rust primitives
            "bool"
                | "char"
                | "str"
                | "String"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
                // Rust common containers (keep the type params, skip the container)
                | "Option"
                | "Result"
                | "Vec"
                | "Box"
                | "Rc"
                | "Arc"
                | "Cell"
                | "RefCell"
                | "Cow"
                | "Pin"
                // TypeScript/JavaScript primitives
                | "string"
                | "number"
                | "boolean"
                | "void"
                | "null"
                | "undefined"
                | "never"
                | "any"
                | "unknown"
                | "object"
                | "symbol"
                | "bigint"
                | "Array"
                | "Promise"
                | "Record"
                | "Map"
                | "Set"
                | "Partial"
                | "Required"
                | "Readonly"
                | "Pick"
                | "Omit"
                // Python primitives
                | "int"
                | "float"
                | "complex"
                | "list"
                | "dict"
                | "set"
                | "tuple"
                | "bytes"
                | "bytearray"
                | "memoryview"
                | "range"
                | "frozenset"
                | "type"
                | "None"
                | "True"
                | "False"
                | "self"
                | "Self"
                | "cls"
                // Java boxed primitives and root types (not user-defined)
                | "Integer"
                | "Long"
                | "Double"
                | "Float"
                | "Short"
                | "Byte"
                | "Character"
                | "Boolean"
                | "Void"
                | "Number"
                | "Object"
        )
    }

    /// Find callers (symbols that call a given function) across all files
    #[allow(dead_code)] // Call graph API - used by index
    pub fn find_callers(
        &mut self,
        root: &Path,
        files: &[(String, bool)],
        symbol_name: &str,
    ) -> Vec<(String, String)> {
        let mut callers = Vec::new();

        for (path, is_dir) in files {
            if *is_dir {
                continue;
            }

            let full_path = root.join(path);
            // Skip files without language support or calls query
            if support_for_path(&full_path).is_none() {
                continue;
            }
            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let symbols = self.parse_file(&full_path, &content);
            for symbol in symbols {
                let callees = self.find_callees_for_symbol(&full_path, &content, &symbol);
                // Check if any callee matches, considering qualifiers
                let is_caller = callees.iter().any(|(name, _, qualifier, _)| {
                    if name != symbol_name {
                        return false;
                    }
                    // Match if: no qualifier, or qualifier is self/Self
                    match qualifier {
                        None => true,
                        Some(q) => q == "self" || q == "Self",
                    }
                });
                if is_caller {
                    callers.push((path.clone(), symbol.name.clone()));
                }
            }
        }

        callers
    }
}

/// Strip surrounding quotes from import path strings (", ', or `).
fn strip_import_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;
    use std::path::PathBuf;

    #[test]
    fn test_parse_python_function() {
        let parser = SymbolParser::new();
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
        let parser = SymbolParser::new();
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
        let parser = SymbolParser::new();
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

    #[test]
    fn test_go_type_refs_struct_fields() {
        let mut parser = SymbolParser::new();
        let content = r#"package main

type Server struct {
    Handler RequestHandler
    Logger  Logger
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("main.go"), content);
        let field_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.kind == TypeRefKind::FieldType)
            .collect();
        assert!(
            field_refs
                .iter()
                .any(|r| r.source_symbol == "Server" && r.target_type == "RequestHandler"),
            "expected Server→RequestHandler field_type"
        );
        assert!(
            field_refs
                .iter()
                .any(|r| r.source_symbol == "Server" && r.target_type == "Logger"),
            "expected Server→Logger field_type"
        );
    }

    #[test]
    fn test_go_type_refs_interface_embed() {
        let mut parser = SymbolParser::new();
        let content = r#"package main

type ReadWriter interface {
    Reader
    Writer
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("main.go"), content);
        let impl_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.kind == TypeRefKind::Implements)
            .collect();
        assert!(
            impl_refs
                .iter()
                .any(|r| r.source_symbol == "ReadWriter" && r.target_type == "Reader"),
            "expected ReadWriter→Reader implements"
        );
        assert!(
            impl_refs
                .iter()
                .any(|r| r.source_symbol == "ReadWriter" && r.target_type == "Writer"),
            "expected ReadWriter→Writer implements"
        );
    }

    #[test]
    fn test_go_type_refs_func_params_return() {
        let mut parser = SymbolParser::new();
        let content = r#"package main

func Process(req Request) Response {
    return Response{}
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("main.go"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ParamType
                && r.source_symbol == "Process"
                && r.target_type == "Request"),
            "expected Process→Request param_type"
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ReturnType
                && r.source_symbol == "Process"
                && r.target_type == "Response"),
            "expected Process→Response return_type"
        );
    }

    #[test]
    fn test_go_type_refs_alias() {
        let mut parser = SymbolParser::new();
        let content = r#"package main

type MyHandler = http.Handler
"#;
        let refs = parser.find_type_refs(&PathBuf::from("main.go"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::TypeAlias
                && r.source_symbol == "MyHandler"
                && r.target_type == "Handler"),
            "expected MyHandler→Handler type_alias (qualified type, leaf name)"
        );
    }

    #[test]
    fn test_java_type_refs_class_hierarchy() {
        let mut parser = SymbolParser::new();
        let content = r#"public class Foo extends Bar implements Baz, Qux {
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Foo.java"), content);
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::Extends && r.target_type == "Bar"),
            "expected Foo extends Bar"
        );
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::Implements && r.target_type == "Baz"),
            "expected Foo implements Baz"
        );
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::Implements && r.target_type == "Qux"),
            "expected Foo implements Qux"
        );
    }

    #[test]
    fn test_java_type_refs_field_and_method() {
        let mut parser = SymbolParser::new();
        let content = r#"public class Service {
    private Repository repo;

    public Response handle(Request req) {
        return null;
    }
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Service.java"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::FieldType
                && r.source_symbol == "Service"
                && r.target_type == "Repository"),
            "expected Service→Repository field_type"
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ReturnType
                && r.source_symbol == "handle"
                && r.target_type == "Response"),
            "expected handle→Response return_type"
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ParamType
                && r.source_symbol == "handle"
                && r.target_type == "Request"),
            "expected handle→Request param_type"
        );
    }

    #[test]
    fn test_java_type_refs_generic_bound() {
        let mut parser = SymbolParser::new();
        let content = r#"public class Sorter {
    public <T extends Comparable> void sort(T[] arr) {}
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Sorter.java"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::GenericBound
                && r.source_symbol == "sort"
                && r.target_type == "Comparable"),
            "expected sort→Comparable generic_bound"
        );
    }

    #[test]
    fn test_java_type_refs_interface_extends() {
        let mut parser = SymbolParser::new();
        let content = r#"interface ReadWriter extends Reader, Writer {
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("ReadWriter.java"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::Extends
                && r.source_symbol == "ReadWriter"
                && r.target_type == "Reader"),
            "expected ReadWriter extends Reader"
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::Extends
                && r.source_symbol == "ReadWriter"
                && r.target_type == "Writer"),
            "expected ReadWriter extends Writer"
        );
    }

    // --- C# type refs ---

    #[test]
    fn test_csharp_type_refs_class_hierarchy() {
        let mut parser = SymbolParser::new();
        let content = r#"class Service : BaseService, IService {
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Service.cs"), content);
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "BaseService"),
            "expected Service : BaseService, got: {:?}",
            refs
        );
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "IService"),
            "expected Service : IService, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_csharp_type_refs_method_return_and_param() {
        let mut parser = SymbolParser::new();
        let content = r#"class Processor {
    private Repository _repo;
    public Response Process(Request req) {
        return null;
    }
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Processor.cs"), content);
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::FieldType && r.target_type == "Repository"),
            "expected field type Repository, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ReturnType
                && r.source_symbol == "Process"
                && r.target_type == "Response"),
            "expected return type Response, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ParamType
                && r.source_symbol == "Process"
                && r.target_type == "Request"),
            "expected param type Request, got: {:?}",
            refs
        );
    }

    // --- Kotlin type refs ---

    #[test]
    fn test_kotlin_type_refs_class_hierarchy() {
        let mut parser = SymbolParser::new();
        let content = r#"class Service : BaseService(), IService {
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Service.kt"), content);
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "BaseService"),
            "expected Service : BaseService, got: {:?}",
            refs
        );
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "IService"),
            "expected Service : IService, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_kotlin_type_refs_function_types() {
        let mut parser = SymbolParser::new();
        let content = r#"class Repo {
    val handler: Handler = Handler()
    fun process(req: Request): Response {
        return Response()
    }
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Repo.kt"), content);
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::FieldType && r.target_type == "Handler"),
            "expected field type Handler, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ParamType
                && r.source_symbol == "process"
                && r.target_type == "Request"),
            "expected param type Request, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ReturnType
                && r.source_symbol == "process"
                && r.target_type == "Response"),
            "expected return type Response, got: {:?}",
            refs
        );
    }

    // --- Swift type refs ---

    #[test]
    fn test_swift_type_refs_class_hierarchy() {
        let mut parser = SymbolParser::new();
        let content = r#"class Service: BaseService, IService {
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Service.swift"), content);
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "BaseService"),
            "expected Service: BaseService, got: {:?}",
            refs
        );
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Service" && r.target_type == "IService"),
            "expected Service: IService, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_swift_type_refs_function_types() {
        let mut parser = SymbolParser::new();
        let content = r#"class Processor {
    func process(req: Request) -> Response {
        return Response()
    }
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("Processor.swift"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ParamType
                && r.source_symbol == "process"
                && r.target_type == "Request"),
            "expected param type Request, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::ReturnType
                && r.source_symbol == "process"
                && r.target_type == "Response"),
            "expected return type Response, got: {:?}",
            refs
        );
    }

    // --- C++ type refs ---

    #[test]
    fn test_cpp_type_refs_class_hierarchy() {
        let mut parser = SymbolParser::new();
        let content = r#"class Derived : public Base, public IFoo {
};
"#;
        let refs = parser.find_type_refs(&PathBuf::from("derived.cpp"), content);
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Derived" && r.target_type == "Base"),
            "expected Derived extends Base, got: {:?}",
            refs
        );
        assert!(
            refs.iter()
                .any(|r| r.source_symbol == "Derived" && r.target_type == "IFoo"),
            "expected Derived extends IFoo, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_cpp_type_refs_function_return_param() {
        let mut parser = SymbolParser::new();
        let content = r#"Response process(Request req) {
    return Response();
}
"#;
        let refs = parser.find_type_refs(&PathBuf::from("proc.cpp"), content);
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::ReturnType && r.target_type == "Response"),
            "expected return type Response, got: {:?}",
            refs
        );
        assert!(
            refs.iter()
                .any(|r| r.kind == TypeRefKind::ParamType && r.target_type == "Request"),
            "expected param type Request, got: {:?}",
            refs
        );
    }

    // --- Ruby type refs ---

    #[test]
    fn test_ruby_type_refs_inheritance() {
        let mut parser = SymbolParser::new();
        let content = r#"class Service < BaseService
end
"#;
        let refs = parser.find_type_refs(&PathBuf::from("service.rb"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::Extends
                && r.source_symbol == "Service"
                && r.target_type == "BaseService"),
            "expected Service < BaseService, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_ruby_type_refs_include() {
        let mut parser = SymbolParser::new();
        let content = r#"class Worker
  include Serializable
  prepend Auditable
end
"#;
        let refs = parser.find_type_refs(&PathBuf::from("worker.rb"), content);
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::Implements
                && r.source_symbol == "Worker"
                && r.target_type == "Serializable"),
            "expected Worker include Serializable, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|r| r.kind == TypeRefKind::Implements
                && r.source_symbol == "Worker"
                && r.target_type == "Auditable"),
            "expected Worker prepend Auditable, got: {:?}",
            refs
        );
    }
}
