use crate::extract::{ExtractOptions, Extractor};
use crate::parsers;
use normalize_facts_core::TypeRef;
use normalize_facts_core::TypeRefKind;
use normalize_languages::{Language, Symbol as LangSymbol, support_for_grammar, support_for_path};
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

    /// Parse imports from any supported language file using trait-based extraction.
    /// Returns a flattened list where each imported name gets its own FlatImport entry.
    pub fn parse_imports(&self, path: &Path, content: &str) -> Vec<FlatImport> {
        let support = match support_for_path(path) {
            Some(s) => s,
            None => return Vec::new(),
        };

        // Check if this language has import support
        if support.import_kinds().is_empty() {
            return Vec::new();
        }

        let tree = match parsers::parse_with_grammar(support.grammar_name(), content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();
        Self::collect_imports_with_trait(&mut cursor, content, support, &mut imports);
        imports
    }

    fn collect_imports_with_trait(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        support: &dyn Language,
        imports: &mut Vec<FlatImport>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            // Check for embedded content (e.g., <script> in Vue/Svelte/HTML)
            if let Some(embedded) = support.embedded_content(&node, content)
                && let Some(sub_lang) = support_for_grammar(embedded.grammar)
                && let Some(sub_tree) =
                    parsers::parse_with_grammar(embedded.grammar, &embedded.content)
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

            // Check for import nodes
            if support.import_kinds().contains(&kind) {
                let lang_imports = support.extract_imports(&node, content);
                // Flatten: each name in the import becomes a separate FlatImport entry
                for lang_imp in lang_imports {
                    if lang_imp.is_wildcard {
                        imports.push(FlatImport {
                            module: Some(lang_imp.module.clone()),
                            name: "*".to_string(),
                            alias: lang_imp.alias.clone(),
                            line: lang_imp.line,
                        });
                    } else if lang_imp.names.is_empty() {
                        // import X (no specific names) - module is the imported thing
                        imports.push(FlatImport {
                            module: None,
                            name: lang_imp.module.clone(),
                            alias: lang_imp.alias.clone(),
                            line: lang_imp.line,
                        });
                    } else {
                        // from X import a, b, c - each name gets an entry
                        for name in &lang_imp.names {
                            imports.push(FlatImport {
                                module: Some(lang_imp.module.clone()),
                                name: name.clone(),
                                alias: None, // alias applies to whole import, not individual names
                                line: lang_imp.line,
                            });
                        }
                    }
                }
            }

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
            calls.into_iter().map(|(name, _, _)| name).collect();
        let mut result: Vec<_> = unique.drain().collect();
        result.sort();
        result
    }

    /// Find callees with line numbers (for call graph indexing)
    /// Returns: (callee_name, line, Option<qualifier>)
    /// For foo.bar(), returns ("bar", line, Some("foo"))
    /// For bar(), returns ("bar", line, None)
    #[allow(dead_code)] // Call graph API - used by index
    pub fn find_callees_with_lines(
        &mut self,
        path: &Path,
        content: &str,
        symbol_name: &str,
    ) -> Vec<(String, usize, Option<String>)> {
        let symbol = match self.find_symbol(path, content, symbol_name) {
            Some(s) => s,
            None => return Vec::new(),
        };
        self.find_callees_for_symbol(path, content, &symbol)
    }

    /// Find callees for a pre-parsed symbol (avoids re-parsing the file)
    /// Use this when you already have the FlatSymbol from parse_file()
    pub fn find_callees_for_symbol(
        &mut self,
        path: &Path,
        content: &str,
        symbol: &FlatSymbol,
    ) -> Vec<(String, usize, Option<String>)> {
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

        let grammar = match loader.get(grammar_name) {
            Some(g) => g,
            None => return Vec::new(),
        };

        let query = match tree_sitter::Query::new(&grammar, &calls_query) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
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

    /// Generic query-based call extraction using `@call` and `@call.qualifier` captures.
    fn collect_calls_with_query(
        root: &tree_sitter::Node,
        source: &str,
        query: &tree_sitter::Query,
        base_line: usize,
    ) -> Vec<(String, usize, Option<String>)> {
        let call_idx = query.capture_names().iter().position(|n| *n == "call");
        let qualifier_idx = query
            .capture_names()
            .iter()
            .position(|n| *n == "call.qualifier");

        let Some(call_idx) = call_idx else {
            return Vec::new();
        };

        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut calls = Vec::new();

        let mut matches = qcursor.matches(query, *root, source.as_bytes());
        while let Some(m) = matches.next() {
            let mut name: Option<(&str, usize)> = None;
            let mut qualifier: Option<&str> = None;

            for capture in m.captures {
                if capture.index as usize == call_idx {
                    let text = &source[capture.node.byte_range()];
                    let line = capture.node.start_position().row + base_line;
                    name = Some((text, line));
                } else if Some(capture.index as usize) == qualifier_idx {
                    qualifier = Some(&source[capture.node.byte_range()]);
                }
            }

            if let Some((call_name, line)) = name {
                calls.push((
                    call_name.to_string(),
                    line,
                    qualifier.map(|q| q.to_string()),
                ));
            }
        }

        calls
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
                | "class_declaration"
                | "interface_declaration"
                | "class_definition"
                | "function_definition" => {
                    if let Some(name_node) = parent.child_by_field_name("name") {
                        return content[name_node.byte_range()].to_string();
                    }
                    // impl_item uses "type" field for the implemented type
                    if parent.kind() == "impl_item"
                        && let Some(type_node) = parent.child_by_field_name("type")
                    {
                        return content[type_node.byte_range()].to_string();
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
                let is_caller = callees.iter().any(|(name, _, qualifier)| {
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
}
