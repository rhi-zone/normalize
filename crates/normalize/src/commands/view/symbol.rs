//! Symbol lookup and rendering for view command.

use super::report::ViewReport;
use crate::skeleton::SymbolExt;
use crate::{parsers, skeleton, symbols};
use normalize_languages::support_for_path;
use std::collections::HashSet;
use std::path::Path;

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
    symbols: &'a [normalize_languages::Symbol],
    name: &str,
) -> Option<&'a normalize_languages::Symbol> {
    find_symbol_ci(symbols, name, false)
}

/// Find a symbol by name in a skeleton (recursive), with case sensitivity control
pub fn find_symbol_ci<'a>(
    symbols: &'a [normalize_languages::Symbol],
    name: &str,
    case_insensitive: bool,
) -> Option<&'a normalize_languages::Symbol> {
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
    symbols: &'a [normalize_languages::Symbol],
    path: &[String],
    case_insensitive: bool,
) -> Option<&'a normalize_languages::Symbol> {
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
    symbol: &'a normalize_languages::Symbol,
    #[allow(dead_code)] // retained for future sibling-count display feature
    sibling_count: usize,
}

/// Result from finding a symbol with its ancestors.
struct SymbolWithAncestors<'a> {
    ancestors: Vec<AncestorInfo<'a>>,
}

/// Find a symbol by name along with all its ancestors (outermost first)
fn find_symbol_with_ancestors<'a>(
    symbols: &'a [normalize_languages::Symbol],
    name: &str,
    ancestors: &mut Vec<AncestorInfo<'a>>,
    case_insensitive: bool,
) -> Option<&'a normalize_languages::Symbol> {
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
    symbols: &'a [normalize_languages::Symbol],
    name: &str,
    case_insensitive: bool,
) -> SymbolWithAncestors<'a> {
    let mut ancestors = Vec::new();
    let _symbol = find_symbol_with_ancestors(symbols, name, &mut ancestors, case_insensitive);
    SymbolWithAncestors { ancestors }
}

/// Find a symbol's signature in a skeleton
pub fn find_symbol_signature(
    symbols: &[normalize_languages::Symbol],
    name: &str,
) -> Option<String> {
    find_symbol(symbols, name).map(|sym| sym.signature.clone())
}

/// Extract identifier names from source code using tree-sitter.
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
) -> Result<ViewReport, String> {
    let matches = crate::path_resolve::resolve_unified_all(file_path, root);
    let resolved = match matches.len() {
        0 => return Err(format!("File not found: {}", file_path)),
        // normalize-syntax-allow: rust/unwrap-in-impl - match arm guards exactly 1 match, so next() is always Some
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
        symbols: &'a [normalize_languages::Symbol],
        line: usize,
        parent: Option<&'a normalize_languages::Symbol>,
    ) -> Option<(
        &'a normalize_languages::Symbol,
        Vec<&'a normalize_languages::Symbol>,
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

    Ok(ViewReport {
        target: full_symbol_path,
        node: view_node,
        source: None,
        imports: Vec::new(),
        exports: Vec::new(),
        parent_signatures,
        line_range: None,
        grammar,
        warnings: Vec::new(),
    })
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
) -> Result<ViewReport, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    let mut parser = symbols::SymbolParser::new();
    // normalize-syntax-allow: rust/unwrap-in-impl - symbol_path non-empty guaranteed by CLI parser
    let symbol_name = symbol_path.last().unwrap();

    let grammar =
        normalize_languages::support_for_path(&full_path).map(|s| s.grammar_name().to_string());

    let deps_result = crate::deps::extract_deps(&full_path, &content);

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

        return Ok(ViewReport {
            target: full_symbol_path,
            node: {
                let path_str = format!("{}/{}", file_path, symbol_path.join("/"));
                let flat_sym = parser.find_symbol(&full_path, &content, symbol_name);
                let sym_kind = flat_sym
                    .as_ref()
                    .map(|s| s.kind.as_str().to_string())
                    .unwrap_or_default();
                crate::tree::ViewNode {
                    name: symbol_name.to_string(),
                    kind: crate::tree::ViewNodeKind::Symbol(sym_kind),
                    path: path_str,
                    children: Vec::new(),
                    signature: None,
                    docstring: None,
                    line_range: match (start_line, end_line) {
                        (Some(s), Some(e)) => Some((s, e)),
                        _ => None,
                    },
                    grammar: grammar.clone(),
                }
            },
            source: Some(source),
            imports,
            exports: Vec::new(),
            parent_signatures,
            line_range: match (start_line, end_line) {
                (Some(s), Some(e)) => Some((s, e)),
                _ => None,
            },
            grammar,
            warnings: Vec::new(),
        });
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

            return Ok(ViewReport {
                target: full_symbol_path.clone(),
                node: sym.to_view_node(&full_symbol_path, grammar.as_deref()),
                source: Some(source),
                imports: Vec::new(),
                exports: Vec::new(),
                parent_signatures,
                line_range: Some((sym.start_line, sym.end_line)),
                grammar,
                warnings: Vec::new(),
            });
        }

        // Fallback: show skeleton node
        let view_node = sym.to_view_node(&full_symbol_path, grammar.as_deref());
        return Ok(ViewReport {
            target: full_symbol_path,
            node: view_node,
            source: None,
            imports: Vec::new(),
            exports: Vec::new(),
            parent_signatures: Vec::new(),
            line_range: None,
            grammar,
            warnings: Vec::new(),
        });
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
) -> Result<Vec<ViewReport>, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    let matches = crate::path_resolve::resolve_symbol_glob(&full_path, &content, pattern);

    if matches.is_empty() {
        return Err(format!("No symbols match pattern: {}", pattern));
    }

    let content_lines: Vec<&str> = content.lines().collect();
    let grammar =
        normalize_languages::support_for_path(&full_path).map(|s| s.grammar_name().to_string());

    let reports = matches
        .iter()
        .map(|m| {
            let source: String = (m.symbol.start_line..=m.symbol.end_line)
                .filter(|&i| i > 0 && i <= content_lines.len())
                .map(|i| content_lines[i - 1])
                .collect::<Vec<_>>()
                .join("\n");
            let sym_path = format!("{}/{}", file_path, m.path);
            let node = crate::tree::ViewNode {
                name: m.symbol.name.clone(),
                kind: crate::tree::ViewNodeKind::Symbol(m.symbol.kind.as_str().to_string()),
                path: sym_path.clone(),
                children: Vec::new(),
                signature: None,
                docstring: None,
                line_range: Some((m.symbol.start_line, m.symbol.end_line)),
                grammar: grammar.clone(),
            };
            ViewReport {
                target: sym_path,
                node,
                source: Some(source),
                imports: Vec::new(),
                exports: Vec::new(),
                parent_signatures: Vec::new(),
                line_range: Some((m.symbol.start_line, m.symbol.end_line)),
                grammar: grammar.clone(),
                warnings: Vec::new(),
            }
        })
        .collect();

    Ok(reports)
}
