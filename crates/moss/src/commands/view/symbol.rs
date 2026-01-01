//! Symbol lookup and rendering for view command.

use crate::skeleton::SymbolExt;
use crate::tree::{DocstringDisplay, FormatOptions};
use crate::{deps, parsers, path_resolve, skeleton, symbols, tree};
use moss_languages::support_for_path;
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
    show_docs: bool,
    show_parent: bool,
    json: bool,
    pretty: bool,
    use_colors: bool,
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
        show_docs,
        show_parent,
        json,
        pretty,
        use_colors,
    )
}

/// View the symbol containing a specific line number
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol_at_line(
    file_path: &str,
    line: usize,
    root: &Path,
    depth: i32,
    show_docs: bool,
    show_parent: bool,
    json: bool,
    pretty: bool,
    use_colors: bool,
) -> i32 {
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
        println!("{}", serde_json::to_string(&view_node).unwrap());
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
            docstrings: if show_docs {
                DocstringDisplay::Full
            } else {
                DocstringDisplay::Summary
            },
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
}

/// Find a symbol by name in a skeleton (recursive)
pub fn find_symbol<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
) -> Option<&'a skeleton::SkeletonSymbol> {
    for sym in symbols {
        if sym.name == name {
            return Some(sym);
        }
        if let Some(found) = find_symbol(&sym.children, name) {
            return Some(found);
        }
    }
    None
}

/// Find a symbol by qualified path (e.g., ["Tsx", "format_import"])
fn find_symbol_by_path<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    path: &[String],
) -> Option<&'a skeleton::SkeletonSymbol> {
    if path.is_empty() {
        return None;
    }

    if path.len() == 1 {
        return find_symbol(symbols, &path[0]);
    }

    let mut current_symbols = symbols;
    for (i, name) in path.iter().enumerate() {
        let found = current_symbols.iter().find(|s| s.name == *name)?;
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

/// Find a symbol by name along with all its ancestors (outermost first)
fn find_symbol_with_ancestors<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    name: &str,
    ancestors: &mut Vec<AncestorInfo<'a>>,
) -> Option<&'a skeleton::SkeletonSymbol> {
    for sym in symbols {
        if sym.name == name {
            return Some(sym);
        }
        for child in &sym.children {
            if child.name == name {
                ancestors.push(AncestorInfo {
                    symbol: sym,
                    sibling_count: sym.children.len().saturating_sub(1),
                });
                return Some(child);
            }
        }
        if let Some(found) = find_symbol_with_ancestors(&sym.children, name, ancestors) {
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
) -> (Option<&'a skeleton::SkeletonSymbol>, Vec<AncestorInfo<'a>>) {
    let mut ancestors = Vec::new();
    let found = find_symbol_with_ancestors(symbols, name, &mut ancestors);
    (found, ancestors)
}

/// Find a symbol's signature in a skeleton
pub fn find_symbol_signature(symbols: &[skeleton::SkeletonSymbol], name: &str) -> Option<String> {
    find_symbol(symbols, name).map(|sym| sym.signature.clone())
}

/// View a symbol within a file
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol(
    file_path: &str,
    symbol_path: &[String],
    root: &Path,
    depth: i32,
    _full: bool,
    show_docs: bool,
    show_parent: bool,
    json: bool,
    pretty: bool,
    use_colors: bool,
) -> i32 {
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
            println!(
                "{}",
                serde_json::json!({
                    "type": "symbol",
                    "path": full_symbol_path,
                    "file": file_path,
                    "symbol": symbol_name,
                    "imports": imports,
                    "source": source
                })
            );
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
            if !deps_result.imports.is_empty() {
                if let Some(ref g) = grammar {
                    let used_ids = extract_identifiers(&source, g);
                    let lang = support_for_path(&full_path);
                    let lines: Vec<&str> = content.lines().collect();
                    let mut seen_imports = HashSet::new();
                    let mut has_imports = false;

                    for import in &deps_result.imports {
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

                        let import_text =
                            if used_names.len() == import.names.len() || import.names.is_empty() {
                                if import.line > 0 && import.line <= lines.len() {
                                    lines[import.line - 1].trim().to_string()
                                } else if let Some(ref l) = lang {
                                    l.format_import(import, None)
                                } else {
                                    import.format_summary()
                                }
                            } else if let Some(ref l) = lang {
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
            }

            // Show ancestor context
            let skeleton_result;
            let ancestors: Vec<(String, usize)> = if show_parent {
                let extractor = skeleton::SkeletonExtractor::new();
                skeleton_result = extractor.extract(&full_path, &content);
                let (_, ancestor_infos) =
                    find_symbol_with_parent(&skeleton_result.symbols, symbol_name);
                ancestor_infos
                    .into_iter()
                    .map(|a| (a.symbol.signature.clone(), a.sibling_count))
                    .collect()
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
                source
            };
            println!("{}", highlighted);

            if let Some((_, sibling_count)) = ancestors.last() {
                if *sibling_count > 0 {
                    println!();
                    println!("    /* {} other members */", sibling_count);
                }
            }
        }
        0
    } else {
        // Try skeleton extraction
        let extractor = skeleton::SkeletonExtractor::new();
        let skeleton_result = extractor.extract(&full_path, &content);

        let found_sym = if symbol_path.len() > 1 {
            find_symbol_by_path(&skeleton_result.symbols, symbol_path)
        } else {
            find_symbol(&skeleton_result.symbols, symbol_name)
        };

        if let Some(sym) = found_sym {
            let full_symbol_path = format!("{}/{}", file_path, symbol_path.join("/"));

            if sym.start_line > 0 && sym.end_line > 0 {
                let lines: Vec<&str> = content.lines().collect();
                let start = sym.start_line - 1;
                let end = std::cmp::min(sym.end_line, lines.len());
                let source: String = lines[start..end].join("\n");

                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "symbol",
                            "path": full_symbol_path,
                            "file": file_path,
                            "symbol": symbol_name,
                            "source": source,
                            "start_line": sym.start_line,
                            "end_line": sym.end_line
                        })
                    );
                } else {
                    if depth >= 0 {
                        println!(
                            "# {} (L{}-{})",
                            full_symbol_path, sym.start_line, sym.end_line
                        );
                    }

                    if show_parent && symbol_path.len() > 1 {
                        if let Some(parent_sym) =
                            find_symbol(&skeleton_result.symbols, &symbol_path[0])
                        {
                            println!("\n{}\n", parent_sym.signature);
                        }
                    }

                    let highlighted = if let Some(ref g) = grammar {
                        tree::highlight_source(&source, g, use_colors)
                    } else {
                        source
                    };
                    println!("{}", highlighted);
                }
                return 0;
            }

            // Fallback: show skeleton
            let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());
            if json {
                println!("{}", serde_json::to_string(&view_node).unwrap());
            } else {
                println!(
                    "# {} ({}, L{}-{})",
                    full_symbol_path,
                    sym.kind.as_str(),
                    sym.start_line,
                    sym.end_line
                );
                let format_options = FormatOptions {
                    docstrings: if show_docs {
                        DocstringDisplay::Full
                    } else {
                        DocstringDisplay::Summary
                    },
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
            // "Did You Mean?" bridge
            let text_matches: Vec<_> = content.match_indices(symbol_name).collect();
            if text_matches.is_empty() {
                eprintln!("Symbol not found: {}", symbol_name);
            } else {
                eprintln!(
                    "Symbol '{}' not found in AST. However, the string '{}' appears {} time{}.",
                    symbol_name,
                    symbol_name,
                    text_matches.len(),
                    if text_matches.len() == 1 { "" } else { "s" }
                );
                eprintln!(
                    "Did you mean: moss text-search '{}' {}",
                    symbol_name, file_path
                );
            }
            1
        }
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

        if kind == "identifier"
            || kind == "type_identifier"
            || kind == "field_identifier"
            || kind == "property_identifier"
            || kind.ends_with("_identifier")
        {
            if let Ok(text) = node.utf8_text(source) {
                identifiers.insert(text.to_string());
            }
        }

        if kind == "scoped_identifier" || kind == "scoped_type_identifier" {
            if let Some(last_child) = node.child(node.child_count().saturating_sub(1)) {
                if let Ok(text) = last_child.utf8_text(source) {
                    identifiers.insert(text.to_string());
                }
            }
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

/// View multiple symbols matching a glob pattern
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_symbol_glob(
    file_path: &str,
    pattern: &str,
    root: &Path,
    _depth: i32,
    _full: bool,
    _show_docs: bool,
    json: bool,
    _pretty: bool,
    _use_colors: bool,
) -> i32 {
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
        let items: Vec<_> = matches
            .iter()
            .map(|m| {
                serde_json::json!({
                    "path": format!("{}/{}", file_path, m.path),
                    "name": m.symbol.name,
                    "kind": m.symbol.kind.as_str(),
                    "start_line": m.symbol.start_line,
                    "end_line": m.symbol.end_line,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "type": "glob_matches",
                "file": file_path,
                "pattern": pattern,
                "count": matches.len(),
                "matches": items
            })
        );
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
