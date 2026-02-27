//! Symbol lookup and rendering for view command.

use super::report::{
    ViewGlobMatch, ViewGlobReport, ViewOutput, ViewSymbolNodeReport, ViewSymbolReport,
};
use crate::output::OutputFormatter;
use crate::skeleton::SymbolExt;
use crate::tree::{DocstringDisplay, FormatOptions};
use crate::{deps, parsers, path_resolve, skeleton, symbols, tree};
use normalize_languages::support_for_path;
use std::collections::HashSet;
use std::path::Path;

/// View a symbol directly by file and name
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol_direct(
    file_path: &str,
    symbol_name: &str,
    parent_name: Option<&str>,
    root: &Path,
    depth: i32,
    full: bool,
    docstring_mode: DocstringDisplay,
    show_parent: bool,
    context: bool,
    format: &crate::output::OutputFormat,
    case_insensitive: bool,
) -> i32 {
    let symbol_path: Vec<String> = match parent_name {
        Some(p) => vec![p.to_string(), symbol_name.to_string()],
        None => vec![symbol_name.to_string()],
    };
    cmd_view_symbol(
        file_path,
        &symbol_path,
        root,
        depth,
        full,
        docstring_mode,
        show_parent,
        context,
        format,
        case_insensitive,
    )
}

/// View the symbol containing a specific line number
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol_at_line(
    file_path: &str,
    line: usize,
    root: &Path,
    depth: i32,
    docstring_mode: DocstringDisplay,
    show_parent: bool,
    context: bool,
    format: &crate::output::OutputFormat,
) -> i32 {
    let json = format.is_json();
    let pretty = format.is_pretty();
    let use_colors = format.use_colors();
    let matches = path_resolve::resolve_unified_all(file_path, root);
    let resolved = match matches.len() {
        0 => {
            eprintln!("File not found: {}", file_path);
            return 1;
        }
        1 => &matches[0],
        _ => {
            eprintln!("Multiple matches for '{}' - be more specific:", file_path);
            for m in &matches {
                println!("  {}", m.file_path);
            }
            return 1;
        }
    };

    if resolved.is_directory {
        eprintln!("Cannot use line number with directory: {}", file_path);
        return 1;
    }

    let full_path = root.join(&resolved.file_path);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", resolved.file_path, e);
            return 1;
        }
    };

    let extractor = skeleton::SkeletonExtractor::new();
    let skeleton_result = extractor.extract(&full_path, &content);

    fn find_symbol_at_line<'a>(
        symbols: &'a [skeleton::SkeletonSymbol],
        line: usize,
        parent: Option<&'a skeleton::SkeletonSymbol>,
    ) -> Option<(
        &'a skeleton::SkeletonSymbol,
        Vec<&'a skeleton::SkeletonSymbol>,
    )> {
        for sym in symbols {
            if let Some((child, mut ancestors)) =
                find_symbol_at_line(&sym.children, line, Some(sym))
            {
                if let Some(p) = parent {
                    ancestors.insert(0, p);
                }
                return Some((child, ancestors));
            }
            if line >= sym.start_line && line <= sym.end_line {
                let mut ancestors = Vec::new();
                if let Some(p) = parent {
                    ancestors.push(p);
                }
                return Some((sym, ancestors));
            }
        }
        None
    }

    let Some((sym, ancestors)) = find_symbol_at_line(&skeleton_result.symbols, line, None) else {
        eprintln!("No symbol found at line {} in {}", line, resolved.file_path);
        return 1;
    };

    let mut symbol_path: Vec<String> = ancestors.iter().map(|a| a.name.clone()).collect();
    symbol_path.push(sym.name.clone());
    let full_symbol_path = format!("{}/{}", resolved.file_path, symbol_path.join("/"));

    let grammar = support_for_path(&full_path).map(|s| s.grammar_name().to_string());
    let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());

    if json {
        let parent_signatures: Vec<String> =
            ancestors.iter().map(|a| a.signature.clone()).collect();
        let report = ViewOutput::SymbolAtLine(ViewSymbolNodeReport {
            node: view_node,
            parent_signatures,
        });
        report.print(format);
    } else {
        if depth >= 0 {
            println!(
                "# {} ({}, L{}-{})",
                full_symbol_path,
                sym.kind.as_str(),
                sym.start_line,
                sym.end_line
            );
        }

        if show_parent {
            for ancestor in &ancestors {
                println!("{}", ancestor.signature);
            }
            if !ancestors.is_empty() {
                println!();
            }
        }

        let format_options = FormatOptions {
            docstrings: docstring_mode,
            line_numbers: true,
            skip_root: false,
            max_depth: None,
            minimal: !pretty,
            use_colors,
        };
        let lines = tree::format_view_node(&view_node, &format_options);
        for line in lines {
            println!("{}", line);
        }

        // Show referenced type definitions when --context is used
        if context && let Some(ref g) = grammar {
            // Extract source for the symbol
            let file_lines: Vec<&str> = content.lines().collect();
            let start = sym.start_line.saturating_sub(1);
            let end = sym.end_line.min(file_lines.len());
            let source = file_lines[start..end].join("\n");

            display_referenced_types(
                &source,
                g,
                &skeleton_result.symbols,
                &sym.name,
                use_colors,
                root,
                &resolved.file_path,
            );
        }
    }
    0
}

/// Check if two names match, optionally case-insensitive
fn names_match(a: &str, b: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Find a symbol by name in a skeleton (recursive)
pub fn find_symbol<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
) -> Option<&'a skeleton::SkeletonSymbol> {
    find_symbol_ci(symbols, name, false)
}

/// Find a symbol by name in a skeleton (recursive), with case sensitivity control
pub fn find_symbol_ci<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
    case_insensitive: bool,
) -> Option<&'a skeleton::SkeletonSymbol> {
    for sym in symbols {
        if names_match(&sym.name, name, case_insensitive) {
            return Some(sym);
        }
        if let Some(found) = find_symbol_ci(&sym.children, name, case_insensitive) {
            return Some(found);
        }
    }
    None
}

/// Find a symbol by qualified path (e.g., ["Tsx", "format_import"])
fn find_symbol_by_path<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    path: &[String],
    case_insensitive: bool,
) -> Option<&'a skeleton::SkeletonSymbol> {
    if path.is_empty() {
        return None;
    }

    if path.len() == 1 {
        return find_symbol_ci(symbols, &path[0], case_insensitive);
    }

    let mut current_symbols = symbols;
    for (i, name) in path.iter().enumerate() {
        let found = current_symbols
            .iter()
            .find(|s| names_match(&s.name, name, case_insensitive))?;
        if i == path.len() - 1 {
            return Some(found);
        }
        current_symbols = &found.children;
    }
    None
}

/// Info about one ancestor in the chain
struct AncestorInfo<'a> {
    symbol: &'a skeleton::SkeletonSymbol,
    sibling_count: usize,
}

/// Result from finding a symbol with its ancestors.
struct SymbolWithAncestors<'a> {
    ancestors: Vec<AncestorInfo<'a>>,
}

/// Find a symbol by name along with all its ancestors (outermost first)
fn find_symbol_with_ancestors<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
    ancestors: &mut Vec<AncestorInfo<'a>>,
    case_insensitive: bool,
) -> Option<&'a skeleton::SkeletonSymbol> {
    for sym in symbols {
        if names_match(&sym.name, name, case_insensitive) {
            return Some(sym);
        }
        for child in &sym.children {
            if names_match(&child.name, name, case_insensitive) {
                ancestors.push(AncestorInfo {
                    symbol: sym,
                    sibling_count: sym.children.len().saturating_sub(1),
                });
                return Some(child);
            }
        }
        if let Some(found) =
            find_symbol_with_ancestors(&sym.children, name, ancestors, case_insensitive)
        {
            ancestors.insert(
                0,
                AncestorInfo {
                    symbol: sym,
                    sibling_count: sym.children.len().saturating_sub(1),
                },
            );
            return Some(found);
        }
    }
    None
}

/// Helper that returns ancestors in a Vec
fn find_symbol_with_parent<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
    case_insensitive: bool,
) -> SymbolWithAncestors<'a> {
    let mut ancestors = Vec::new();
    let _symbol = find_symbol_with_ancestors(symbols, name, &mut ancestors, case_insensitive);
    SymbolWithAncestors { ancestors }
}

/// Find a symbol's signature in a skeleton
pub fn find_symbol_signature(symbols: &[skeleton::SkeletonSymbol], name: &str) -> Option<String> {
    find_symbol(symbols, name).map(|sym| sym.signature.clone())
}

/// Print smart header imports: only imports used by the given source
fn print_smart_imports(
    source: &str,
    grammar: &str,
    full_path: &Path,
    content: &str,
    imports: &[normalize_languages::Import],
) {
    let used_ids = extract_identifiers(source, grammar);
    let lang = support_for_path(full_path);
    let lines: Vec<&str> = content.lines().collect();
    let mut seen_imports = HashSet::new();
    let mut has_imports = false;

    for import in imports {
        let used_names: Vec<&str> = import
            .names
            .iter()
            .filter(|n| used_ids.contains(*n))
            .map(|s| s.as_str())
            .collect();

        let module_used = used_ids.contains(&import.module)
            || import
                .module
                .rsplit("::")
                .next()
                .map(|last| used_ids.contains(last))
                .unwrap_or(false);

        if used_names.is_empty() && !module_used && !import.is_wildcard {
            continue;
        }

        let import_text = if used_names.len() == import.names.len() || import.names.is_empty() {
            if import.line > 0 && import.line <= lines.len() {
                lines[import.line - 1].trim().to_string()
            } else if let Some(l) = lang {
                l.format_import(import, None)
            } else {
                import.format_summary()
            }
        } else if let Some(l) = lang {
            l.format_import(import, Some(&used_names))
        } else {
            import.format_summary()
        };

        if seen_imports.insert(import_text.clone()) {
            if !has_imports {
                println!();
                has_imports = true;
            }
            println!("{}", import_text);
        }
    }

    if has_imports {
        println!();
    }
}

/// Build smart imports as a String (for service layer).
fn format_smart_imports_str(
    source: &str,
    grammar: &str,
    full_path: &Path,
    content: &str,
    imports: &[normalize_languages::Import],
) -> String {
    let used_ids = extract_identifiers(source, grammar);
    let lang = support_for_path(full_path);
    let lines: Vec<&str> = content.lines().collect();
    let mut seen_imports = HashSet::new();
    let mut result = String::new();
    let mut has_imports = false;

    for import in imports {
        let used_names: Vec<&str> = import
            .names
            .iter()
            .filter(|n| used_ids.contains(*n))
            .map(|s| s.as_str())
            .collect();

        let module_used = used_ids.contains(&import.module)
            || import
                .module
                .rsplit("::")
                .next()
                .map(|last| used_ids.contains(last))
                .unwrap_or(false);

        if used_names.is_empty() && !module_used && !import.is_wildcard {
            continue;
        }

        let import_text = if used_names.len() == import.names.len() || import.names.is_empty() {
            if import.line > 0 && import.line <= lines.len() {
                lines[import.line - 1].trim().to_string()
            } else if let Some(l) = lang {
                l.format_import(import, None)
            } else {
                import.format_summary()
            }
        } else if let Some(l) = lang {
            l.format_import(import, Some(&used_names))
        } else {
            import.format_summary()
        };

        if seen_imports.insert(import_text.clone()) {
            if !has_imports {
                result.push('\n');
                has_imports = true;
            }
            result.push_str(&import_text);
            result.push('\n');
        }
    }

    if has_imports {
        result.push('\n');
    }
    result
}

/// View a symbol within a file
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol(
    file_path: &str,
    symbol_path: &[String],
    root: &Path,
    depth: i32,
    _full: bool,
    docstring_mode: DocstringDisplay,
    show_parent: bool,
    context: bool,
    format: &crate::output::OutputFormat,
    case_insensitive: bool,
) -> i32 {
    let json = format.is_json();
    let pretty = format.is_pretty();
    let use_colors = format.use_colors();
    let full_path = root.join(file_path);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", file_path, e);
            return 1;
        }
    };

    let mut parser = symbols::SymbolParser::new();
    let symbol_name = symbol_path.last().unwrap();

    let grammar = support_for_path(&full_path).map(|s| s.grammar_name().to_string());

    let deps_extractor = deps::DepsExtractor::new();
    let deps_result = deps_extractor.extract(&full_path, &content);

    // Try fast path for single-element paths
    let source_opt = if symbol_path.len() == 1 {
        parser.extract_symbol_source(&full_path, &content, symbol_name)
    } else {
        None
    };

    if let Some(source) = source_opt {
        let full_symbol_path = format!("{}/{}", file_path, symbol_path.join("/"));

        if json {
            let imports: Vec<String> = deps_result
                .imports
                .iter()
                .map(|i| i.format_summary())
                .collect();
            let report = ViewOutput::Symbol(ViewSymbolReport {
                path: full_symbol_path.clone(),
                file: file_path.to_string(),
                symbol: symbol_name.to_string(),
                imports: Some(imports),
                source: Some(source.clone()),
                start_line: None,
                end_line: None,
                grammar: grammar.clone(),
                parent_signatures: vec![],
            });
            report.print(format);
        } else {
            if depth >= 0 {
                if let Some(sym) = parser.find_symbol(&full_path, &content, symbol_name) {
                    println!(
                        "# {} (L{}-{})",
                        full_symbol_path, sym.start_line, sym.end_line
                    );
                } else {
                    println!("# {}", full_symbol_path);
                }
            }

            // Smart Header: show only imports used by this symbol
            if !deps_result.imports.is_empty()
                && let Some(ref g) = grammar
            {
                print_smart_imports(&source, g, &full_path, &content, &deps_result.imports);
            }

            // Show ancestor context (extract skeleton if needed for parent or context)
            let skeleton_result = if show_parent || context {
                let extractor = skeleton::SkeletonExtractor::new();
                Some(extractor.extract(&full_path, &content))
            } else {
                None
            };

            let ancestors: Vec<(String, usize)> = if show_parent {
                if let Some(ref sr) = skeleton_result {
                    let result =
                        find_symbol_with_parent(&sr.symbols, symbol_name, case_insensitive);
                    result
                        .ancestors
                        .into_iter()
                        .map(|a| (a.symbol.signature.clone(), a.sibling_count))
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            for (signature, _) in &ancestors {
                println!("{}", signature);
            }
            if !ancestors.is_empty() {
                println!();
            }

            let highlighted = if let Some(ref g) = grammar {
                tree::highlight_source(&source, g, use_colors)
            } else {
                source.clone()
            };
            println!("{}", highlighted);

            if let Some((_, sibling_count)) = ancestors.last()
                && *sibling_count > 0
            {
                println!();
                println!("    /* {} other members */", sibling_count);
            }

            // Show referenced type definitions when --context is used
            if context && let (Some(sr), Some(g)) = (&skeleton_result, &grammar) {
                display_referenced_types(
                    &source,
                    g,
                    &sr.symbols,
                    symbol_name,
                    use_colors,
                    root,
                    file_path,
                );
            }
        }
        0
    } else {
        // Try skeleton extraction
        let extractor = skeleton::SkeletonExtractor::new();
        let skeleton_result = extractor.extract(&full_path, &content);

        let found_sym = if symbol_path.len() > 1 {
            find_symbol_by_path(&skeleton_result.symbols, symbol_path, case_insensitive)
        } else {
            find_symbol_ci(&skeleton_result.symbols, symbol_name, case_insensitive)
        };

        if let Some(sym) = found_sym {
            let full_symbol_path = format!("{}/{}", file_path, symbol_path.join("/"));

            if sym.start_line > 0 && sym.end_line > 0 {
                let lines: Vec<&str> = content.lines().collect();
                let start = sym.start_line - 1;
                let end = std::cmp::min(sym.end_line, lines.len());
                let source: String = lines[start..end].join("\n");

                if json {
                    let report = ViewOutput::Symbol(ViewSymbolReport {
                        path: full_symbol_path.clone(),
                        file: file_path.to_string(),
                        symbol: symbol_name.to_string(),
                        imports: None,
                        source: Some(source.clone()),
                        start_line: Some(sym.start_line),
                        end_line: Some(sym.end_line),
                        grammar: grammar.clone(),
                        parent_signatures: vec![],
                    });
                    report.print(format);
                } else {
                    if depth >= 0 {
                        println!(
                            "# {} (L{}-{})",
                            full_symbol_path, sym.start_line, sym.end_line
                        );
                    }

                    if show_parent
                        && symbol_path.len() > 1
                        && let Some(parent_sym) = find_symbol_ci(
                            &skeleton_result.symbols,
                            &symbol_path[0],
                            case_insensitive,
                        )
                    {
                        println!("\n{}\n", parent_sym.signature);
                    }

                    let highlighted = if let Some(ref g) = grammar {
                        tree::highlight_source(&source, g, use_colors)
                    } else {
                        source.clone()
                    };
                    println!("{}", highlighted);

                    // Show referenced type definitions when --context is used
                    if context && let Some(ref g) = grammar {
                        display_referenced_types(
                            &source,
                            g,
                            &skeleton_result.symbols,
                            symbol_name,
                            use_colors,
                            root,
                            file_path,
                        );
                    }
                }
                return 0;
            }

            // Fallback: show skeleton
            let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());
            if json {
                let report = ViewOutput::SymbolAtLine(ViewSymbolNodeReport {
                    node: view_node,
                    parent_signatures: vec![],
                });
                report.print(format);
            } else {
                println!(
                    "# {} ({}, L{}-{})",
                    full_symbol_path,
                    sym.kind.as_str(),
                    sym.start_line,
                    sym.end_line
                );
                let format_options = FormatOptions {
                    docstrings: docstring_mode,
                    line_numbers: true,
                    skip_root: false,
                    max_depth: None,
                    minimal: !pretty,
                    use_colors,
                };
                let lines = tree::format_view_node(&view_node, &format_options);
                for line in lines {
                    println!("{}", line);
                }
            }
            0
        } else {
            // Suggest close matches via trigram containment.
            const TRIGRAM_THRESHOLD: f64 = 0.5;
            const MIN_QUERY_LEN: usize = 4;

            let mut all_names = Vec::new();
            collect_symbol_names(&skeleton_result.symbols, &mut all_names);

            let suggestions: Vec<&str> = if symbol_name.len() >= MIN_QUERY_LEN {
                let mut scored: Vec<(&str, f64)> = all_names
                    .iter()
                    .map(|n| (n.as_str(), trigram_containment(symbol_name, n)))
                    .filter(|(_, score)| *score >= TRIGRAM_THRESHOLD)
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                scored.into_iter().map(|(n, _)| n).collect()
            } else {
                Vec::new()
            };

            if suggestions.is_empty() {
                eprintln!("Symbol '{}' not found in {}", symbol_name, file_path);
            } else {
                eprintln!(
                    "Symbol '{}' not found in {}. Did you mean:",
                    symbol_name, file_path
                );
                for name in &suggestions {
                    eprintln!("  normalize view {}/{}", file_path, name);
                }
            }
            1
        }
    }
}

/// Trigram containment: fraction of query's character trigrams that appear in candidate.
/// Asymmetric by design — measures how much of the query is "present in" the candidate.
fn trigram_containment(query: &str, candidate: &str) -> f64 {
    if query.len() < 3 {
        return 0.0;
    }
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();
    let query_trigrams: HashSet<[u8; 3]> = q
        .as_bytes()
        .windows(3)
        .map(|w| [w[0], w[1], w[2]])
        .collect();
    let candidate_trigrams: HashSet<[u8; 3]> = c
        .as_bytes()
        .windows(3)
        .map(|w| [w[0], w[1], w[2]])
        .collect();
    let matches = query_trigrams.intersection(&candidate_trigrams).count();
    matches as f64 / query_trigrams.len() as f64
}

/// Collect all symbol names (flat) from a symbol tree.
fn collect_symbol_names(symbols: &[normalize_languages::Symbol], out: &mut Vec<String>) {
    for sym in symbols {
        out.push(sym.name.clone());
        collect_symbol_names(&sym.children, out);
    }
}

/// Extract all identifiers used in source code.
fn extract_identifiers(source: &str, grammar: &str) -> HashSet<String> {
    let mut identifiers = HashSet::new();

    if let Some(tree) = parsers::parse_with_grammar(grammar, source) {
        let mut cursor = tree.walk();
        collect_identifiers(&mut cursor, source.as_bytes(), &mut identifiers);
    }

    identifiers
}

/// Recursively collect identifiers from AST.
fn collect_identifiers(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    identifiers: &mut HashSet<String>,
) {
    loop {
        let node = cursor.node();
        let kind = node.kind();

        if (kind == "identifier"
            || kind == "type_identifier"
            || kind == "field_identifier"
            || kind == "property_identifier"
            || kind.ends_with("_identifier"))
            && let Ok(text) = node.utf8_text(source)
        {
            identifiers.insert(text.to_string());
        }

        if (kind == "scoped_identifier" || kind == "scoped_type_identifier")
            && let Some(last_child) = node.child(node.child_count().saturating_sub(1) as u32)
            && let Ok(text) = last_child.utf8_text(source)
        {
            identifiers.insert(text.to_string());
        }

        if cursor.goto_first_child() {
            collect_identifiers(cursor, source, identifiers);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Extract type identifiers from source code (for --context feature).
/// Returns a set of type names referenced in the source.
fn extract_type_references(source: &str, grammar: &str) -> HashSet<String> {
    let mut types = HashSet::new();

    if let Some(tree) = parsers::parse_with_grammar(grammar, source) {
        let mut cursor = tree.walk();
        collect_type_identifiers(&mut cursor, source.as_bytes(), &mut types);
    }

    types
}

/// Recursively collect only type identifiers from AST.
fn collect_type_identifiers(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    types: &mut HashSet<String>,
) {
    loop {
        let node = cursor.node();
        let kind = node.kind();

        // Collect type identifier nodes
        if kind == "type_identifier"
            && let Ok(text) = node.utf8_text(source)
        {
            types.insert(text.to_string());
        }

        // For scoped types like std::Vec, extract the last component
        if kind == "scoped_type_identifier"
            && let Some(last_child) = node.child(node.child_count().saturating_sub(1) as u32)
            && let Ok(text) = last_child.utf8_text(source)
        {
            types.insert(text.to_string());
        }

        // Generic type arguments (e.g., T in Vec<T>)
        if kind == "generic_type"
            // First child is usually the type name
            && let Some(first_child) = node.child(0)
            && let Ok(text) = first_child.utf8_text(source)
        {
            types.insert(text.to_string());
        }

        if cursor.goto_first_child() {
            collect_type_identifiers(cursor, source, types);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Find type definitions in skeleton that match the given type names.
/// Returns symbols that are type definitions (struct, enum, type alias, trait, interface, class).
fn find_type_definitions<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    type_names: &HashSet<String>,
) -> Vec<&'a skeleton::SkeletonSymbol> {
    let mut found = Vec::new();

    for sym in symbols {
        // Check if this is a type definition
        let is_type_def = matches!(
            sym.kind,
            normalize_languages::SymbolKind::Struct
                | normalize_languages::SymbolKind::Enum
                | normalize_languages::SymbolKind::Type
                | normalize_languages::SymbolKind::Trait
                | normalize_languages::SymbolKind::Interface
                | normalize_languages::SymbolKind::Class
        );

        if is_type_def && type_names.contains(&sym.name) {
            found.push(sym);
        }

        // Recurse into children
        found.extend(find_type_definitions(&sym.children, type_names));
    }

    found
}

/// Display referenced type definitions for --context feature.
/// Shows types from the same file first, then cross-file types via index.
fn display_referenced_types(
    source: &str,
    grammar: &str,
    symbols: &[skeleton::SkeletonSymbol],
    symbol_name: &str,
    use_colors: bool,
    root: &Path,
    current_file: &str,
) {
    let type_refs = extract_type_references(source, grammar);

    // Exclude the symbol itself from type references
    let mut type_refs = type_refs;
    type_refs.remove(symbol_name);

    if type_refs.is_empty() {
        return;
    }

    // Find types in same file
    let local_type_defs = find_type_definitions(symbols, &type_refs);
    let local_names: HashSet<String> = local_type_defs.iter().map(|s| s.name.clone()).collect();

    // Find remaining types not found locally via index
    let remaining: HashSet<String> = type_refs.difference(&local_names).cloned().collect();

    let mut external_types: Vec<(String, String, String, usize)> = Vec::new(); // (name, file, signature, line)

    if !remaining.is_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        if let Some(idx) = rt.block_on(crate::index::open_if_enabled(root)) {
            for type_name in &remaining {
                if let Ok(matches) = rt.block_on(idx.find_symbol(type_name)) {
                    // Find first match that's a type definition (not from current file)
                    for (file, kind, start_line, _end_line) in matches {
                        // Skip if from current file (already checked locally)
                        if file == current_file {
                            continue;
                        }
                        // Only include type-defining symbols
                        if !["struct", "enum", "type", "trait", "interface", "class"]
                            .contains(&kind.as_str())
                        {
                            continue;
                        }
                        // Fetch signature from file
                        let full_path = root.join(&file);
                        if let Ok(content) = std::fs::read_to_string(&full_path) {
                            let extractor = skeleton::SkeletonExtractor::new();
                            let result = extractor.extract(&full_path, &content);
                            if let Some(sym) = find_symbol(&result.symbols, type_name) {
                                external_types.push((
                                    type_name.clone(),
                                    file.clone(),
                                    sym.signature.clone(),
                                    start_line,
                                ));
                                break; // Found it, move to next type
                            }
                        }
                    }
                }
            }
        }
    }

    if local_type_defs.is_empty() && external_types.is_empty() {
        return;
    }

    println!();
    println!("// Referenced types:");

    // Show local types first
    for sym in local_type_defs {
        let highlighted = tree::highlight_source(&sym.signature, grammar, use_colors);
        println!("//   {} (L{})", highlighted.trim(), sym.start_line);
    }

    // Show external types with file path
    for (_name, file, signature, line) in external_types {
        let file_grammar = support_for_path(Path::new(&file)).map(|s| s.grammar_name().to_string());
        let highlighted = if let Some(ref g) = file_grammar {
            tree::highlight_source(&signature, g, use_colors)
        } else {
            signature
        };
        println!("//   {} ({}:{})", highlighted.trim(), file, line);
    }
}

/// View multiple symbols matching a glob pattern
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol_glob(
    file_path: &str,
    pattern: &str,
    root: &Path,
    _depth: i32,
    _full: bool,
    _docstring_mode: DocstringDisplay,
    format: &crate::output::OutputFormat,
    _case_insensitive: bool,
) -> i32 {
    let json = format.is_json();
    let full_path = root.join(file_path);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", file_path, e);
            return 1;
        }
    };

    let matches = path_resolve::resolve_symbol_glob(&full_path, &content, pattern);

    if matches.is_empty() {
        eprintln!("No symbols match pattern: {}", pattern);
        return 1;
    }

    if json {
        let file_lines: Vec<&str> = content.lines().collect();
        let report = ViewOutput::GlobMatches(ViewGlobReport {
            file: file_path.to_string(),
            pattern: pattern.to_string(),
            count: matches.len(),
            matches: matches
                .iter()
                .map(|m| {
                    let start = m.symbol.start_line.saturating_sub(1);
                    let end = m.symbol.end_line.min(file_lines.len());
                    let source = file_lines[start..end].join("\n");
                    ViewGlobMatch {
                        path: format!("{}/{}", file_path, m.path),
                        name: m.symbol.name.clone(),
                        kind: m.symbol.kind.as_str().to_string(),
                        start_line: m.symbol.start_line,
                        end_line: m.symbol.end_line,
                        source,
                    }
                })
                .collect(),
        });
        report.print(format);
        return 0;
    }

    println!("# {}/{} ({} matches)", file_path, pattern, matches.len());
    println!();

    let lines: Vec<&str> = content.lines().collect();

    // Show each matched symbol
    for m in &matches {
        println!(
            "## {} ({}, L{}-{})",
            m.path,
            m.symbol.kind.as_str(),
            m.symbol.start_line,
            m.symbol.end_line
        );

        // Show symbol source lines
        for i in m.symbol.start_line..=m.symbol.end_line {
            if i > 0 && i <= lines.len() {
                println!("{}", lines[i - 1]);
            }
        }
        println!();
    }

    0
}

// ─── Service-layer build functions ──────────────────────────────────────────

/// Build symbol-at-line view for the service layer.
#[allow(clippy::too_many_arguments)]
pub fn build_view_symbol_at_line_service(
    file_path: &str,
    line: usize,
    root: &Path,
    _depth: i32,
    _docstring_mode: crate::tree::DocstringDisplay,
    show_parent: bool,
    _context: bool,
) -> Result<ViewOutput, String> {
    let matches = crate::path_resolve::resolve_unified_all(file_path, root);
    let resolved = match matches.len() {
        0 => return Err(format!("File not found: {}", file_path)),
        1 => matches.into_iter().next().unwrap(),
        _ => {
            return Err(format!(
                "Multiple matches for '{}' - be more specific",
                file_path
            ));
        }
    };

    if resolved.is_directory {
        return Err(format!(
            "Cannot use line number with directory: {}",
            file_path
        ));
    }

    let full_path = root.join(&resolved.file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", resolved.file_path, e))?;

    let extractor = skeleton::SkeletonExtractor::new();
    let skeleton_result = extractor.extract(&full_path, &content);

    fn find_at_line<'a>(
        symbols: &'a [skeleton::SkeletonSymbol],
        line: usize,
        parent: Option<&'a skeleton::SkeletonSymbol>,
    ) -> Option<(
        &'a skeleton::SkeletonSymbol,
        Vec<&'a skeleton::SkeletonSymbol>,
    )> {
        for sym in symbols {
            if let Some((child, mut ancestors)) = find_at_line(&sym.children, line, Some(sym)) {
                if let Some(p) = parent {
                    ancestors.insert(0, p);
                }
                return Some((child, ancestors));
            }
            if line >= sym.start_line && line <= sym.end_line {
                let mut ancestors = Vec::new();
                if let Some(p) = parent {
                    ancestors.push(p);
                }
                return Some((sym, ancestors));
            }
        }
        None
    }

    let Some((sym, ancestors)) = find_at_line(&skeleton_result.symbols, line, None) else {
        return Err(format!(
            "No symbol found at line {} in {}",
            line, resolved.file_path
        ));
    };

    let mut symbol_path_parts: Vec<String> = ancestors.iter().map(|a| a.name.clone()).collect();
    symbol_path_parts.push(sym.name.clone());
    let full_symbol_path = format!("{}/{}", resolved.file_path, symbol_path_parts.join("/"));

    let grammar =
        normalize_languages::support_for_path(&full_path).map(|s| s.grammar_name().to_string());
    let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());

    let parent_signatures = if show_parent {
        ancestors.iter().map(|a| a.signature.clone()).collect()
    } else {
        Vec::new()
    };

    Ok(ViewOutput::SymbolAtLine(ViewSymbolNodeReport {
        node: view_node,
        parent_signatures,
    }))
}

/// Build symbol view for the service layer.
#[allow(clippy::too_many_arguments)]
pub fn build_view_symbol_service(
    file_path: &str,
    symbol_path: &[String],
    root: &Path,
    _depth: i32,
    _full: bool,
    _docstring_mode: crate::tree::DocstringDisplay,
    show_parent: bool,
    _context: bool,
    case_insensitive: bool,
) -> Result<ViewOutput, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    let mut parser = symbols::SymbolParser::new();
    let symbol_name = symbol_path.last().unwrap();

    let grammar =
        normalize_languages::support_for_path(&full_path).map(|s| s.grammar_name().to_string());

    let deps_extractor = crate::deps::DepsExtractor::new();
    let deps_result = deps_extractor.extract(&full_path, &content);

    // Try fast path for single-element paths
    let source_opt = if symbol_path.len() == 1 {
        parser.extract_symbol_source(&full_path, &content, symbol_name)
    } else {
        None
    };

    if let Some(source) = source_opt {
        let full_symbol_path = format!("{}/{}", file_path, symbol_path.join("/"));

        let imports: Vec<String> = if !deps_result.imports.is_empty()
            && let Some(ref g) = grammar
        {
            format_smart_imports_str(&source, g, &full_path, &content, &deps_result.imports)
                .lines()
                .map(|l| l.to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let (start_line, end_line) = parser
            .find_symbol(&full_path, &content, symbol_name)
            .map(|sym| (Some(sym.start_line), Some(sym.end_line)))
            .unwrap_or((None, None));

        let parent_signatures = if show_parent {
            let extractor = skeleton::SkeletonExtractor::new();
            let sr = extractor.extract(&full_path, &content);
            let result = find_symbol_with_parent(&sr.symbols, symbol_name, case_insensitive);
            result
                .ancestors
                .into_iter()
                .map(|a| a.symbol.signature.clone())
                .collect()
        } else {
            Vec::new()
        };

        return Ok(ViewOutput::Symbol(ViewSymbolReport {
            path: full_symbol_path,
            file: file_path.to_string(),
            symbol: symbol_name.to_string(),
            imports: Some(imports),
            source: Some(source),
            start_line,
            end_line,
            grammar,
            parent_signatures,
        }));
    }

    // Skeleton extraction path
    let extractor = skeleton::SkeletonExtractor::new();
    let skeleton_result = extractor.extract(&full_path, &content);

    let found_sym = if symbol_path.len() > 1 {
        find_symbol_by_path(&skeleton_result.symbols, symbol_path, case_insensitive)
    } else {
        find_symbol_ci(&skeleton_result.symbols, symbol_name, case_insensitive)
    };

    if let Some(sym) = found_sym {
        let full_symbol_path = format!("{}/{}", file_path, symbol_path.join("/"));

        if sym.start_line > 0 && sym.end_line > 0 {
            let lines: Vec<&str> = content.lines().collect();
            let start = sym.start_line - 1;
            let end = std::cmp::min(sym.end_line, lines.len());
            let source: String = lines[start..end].join("\n");

            let parent_signatures = if show_parent && symbol_path.len() > 1 {
                find_symbol_ci(&skeleton_result.symbols, &symbol_path[0], case_insensitive)
                    .map(|p| vec![p.signature.clone()])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            return Ok(ViewOutput::Symbol(ViewSymbolReport {
                path: full_symbol_path,
                file: file_path.to_string(),
                symbol: symbol_name.to_string(),
                imports: None,
                source: Some(source),
                start_line: Some(sym.start_line),
                end_line: Some(sym.end_line),
                grammar,
                parent_signatures,
            }));
        }

        // Fallback: show skeleton node
        let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());
        return Ok(ViewOutput::SymbolAtLine(ViewSymbolNodeReport {
            node: view_node,
            parent_signatures: Vec::new(),
        }));
    }

    Err(format!(
        "Symbol '{}' not found in {}",
        symbol_name, file_path
    ))
}

/// Build glob-matched symbols view for the service layer.
pub fn build_view_symbol_glob_service(
    file_path: &str,
    pattern: &str,
    root: &Path,
) -> Result<ViewOutput, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    let matches = crate::path_resolve::resolve_symbol_glob(&full_path, &content, pattern);

    if matches.is_empty() {
        return Err(format!("No symbols match pattern: {}", pattern));
    }

    let content_lines: Vec<&str> = content.lines().collect();

    Ok(ViewOutput::GlobMatches(ViewGlobReport {
        file: file_path.to_string(),
        pattern: pattern.to_string(),
        count: matches.len(),
        matches: matches
            .iter()
            .map(|m| {
                let source: String = (m.symbol.start_line..=m.symbol.end_line)
                    .filter(|&i| i > 0 && i <= content_lines.len())
                    .map(|i| content_lines[i - 1])
                    .collect::<Vec<_>>()
                    .join("\n");
                ViewGlobMatch {
                    path: format!("{}/{}", file_path, m.path),
                    name: m.symbol.name.clone(),
                    kind: m.symbol.kind.as_str().to_string(),
                    start_line: m.symbol.start_line,
                    end_line: m.symbol.end_line,
                    source,
                }
            })
            .collect(),
    }))
}
