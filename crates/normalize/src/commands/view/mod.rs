//! View command - unified view of files, directories, and symbols.

pub mod file;
pub mod history;
pub mod lines;
pub mod report;
pub mod search;
pub mod symbol;
pub mod tree;

use crate::tree::DocstringDisplay;
use crate::{daemon, path_resolve};
use serde::Deserialize;
use std::path::Path;

pub use search::search_symbols;

/// View command configuration.
#[derive(
    Debug, Clone, Deserialize, serde::Serialize, Default, schemars::JsonSchema, server_less::Config,
)]
#[serde(default)]
pub struct ViewConfig {
    /// Default depth for tree expansion (0=names, 1=signatures, 2=children, -1=all)
    pub depth: Option<i32>,
    /// Show line numbers by default
    pub line_numbers: Option<bool>,
    /// Show full docstrings by default (vs summary)
    pub show_docs: Option<bool>,
}

impl ViewConfig {
    pub fn depth(&self) -> i32 {
        self.depth.unwrap_or(1)
    }

    pub fn line_numbers(&self) -> bool {
        self.line_numbers.unwrap_or(true)
    }

    pub fn show_docs(&self) -> bool {
        self.show_docs.unwrap_or(false)
    }
}

/// Build a view result for the service layer.
///
/// Routes to the appropriate sub-function based on the target and options,
/// returning a typed ViewReport instead of writing directly to stdout.
#[allow(clippy::too_many_arguments)]
pub async fn build_view_service(
    target: Option<&str>,
    root: &Path,
    depth: i32,
    _line_numbers: bool,
    _show_deps: bool,
    kind_filter: Option<&tree::SymbolKindFilter>,
    types_only: bool,
    show_tests: bool,
    raw: bool,
    _focus: Option<&str>,
    _resolve_imports: bool,
    full: bool,
    docstring_mode: DocstringDisplay,
    context: bool,
    show_parent: bool,
    exclude: &[String],
    only: &[String],
    case_insensitive: bool,
) -> Result<report::ViewReport, String> {
    // Ensure daemon is running if configured
    daemon::maybe_start_daemon(root);

    // Build filter if exclude/only patterns are specified
    let filter = super::build_filter(root, exclude, only);

    // If kind filter is specified, route to list service and error if multiple
    if let Some(kind) = kind_filter {
        let scope = target.unwrap_or(".");
        let results = tree::build_view_filtered_service(root, scope, kind)?;
        return if results.len() == 1 {
            // normalize-syntax-allow: rust/unwrap-in-impl - len == 1 guarantees Some
            Ok(results.into_iter().next().unwrap())
        } else if results.is_empty() {
            Err(format!("No symbols found matching type: {}", kind))
        } else {
            let mut msg = format!(
                "Multiple matches for kind '{}' - use 'normalize view list --kind {}' to list all:\n",
                kind, kind
            );
            for r in &results {
                msg.push_str(&format!("  {}\n", r.target));
            }
            Err(msg)
        };
    }

    let target_str = target.unwrap_or(".");

    // Handle "." as current directory
    if target_str == "." {
        return tree::build_view_directory_service(root, depth, raw, filter.as_ref());
    }

    // Handle line targets: file.rs:30 (symbol at line) or file.rs:30-55 (range)
    if let Some((file_path, line, end_opt)) = lines::parse_line_target(target_str) {
        if let Some(end) = end_opt {
            return lines::build_view_line_range_service(
                &file_path,
                line,
                end,
                root,
                docstring_mode,
            );
        } else {
            return symbol::build_view_symbol_at_line_service(
                &file_path,
                line,
                root,
                depth,
                docstring_mode,
                show_parent,
                context,
            );
        }
    }

    // Detect symbol query vs path
    let has_file_extension = target_str
        .rsplit('/')
        .next()
        .map(|last| last.contains('.'))
        .unwrap_or(false);
    let dir_only = target_str.ends_with('/');

    let is_symbol_query = !dir_only
        && !target_str.starts_with('@')
        && target_str.contains('/')
        && !target_str.starts_with('/')
        && !has_file_extension
        && {
            let first_seg = target_str.split('/').next().unwrap_or("");
            !root.join(first_seg).exists()
        };

    let (matches, symbol_matches) = if is_symbol_query {
        (Vec::new(), search::search_symbols(target_str, root).await)
    } else {
        let matches = path_resolve::resolve_unified_all(target_str, root);
        let symbol_matches = if matches.is_empty() && !dir_only {
            search::search_symbols(target_str, root).await
        } else {
            Vec::new()
        };
        (matches, symbol_matches)
    };

    let unified = match (matches.len(), symbol_matches.len()) {
        (0, 0) => {
            let mut msg = if has_file_extension || dir_only {
                // Looks like a path — give a more actionable message
                format!(
                    "Path not found: {} (check that the path exists and is not ignored)",
                    target_str
                )
            } else {
                format!("No matches for: {}", target_str)
            };
            let suggestions = search::suggest_symbols_trigram(target_str, root, 0.5, 5);
            if !suggestions.is_empty() {
                msg.push_str("\nDid you mean?");
                for (sym, _score) in suggestions {
                    let prefix = sym
                        .parent
                        .as_deref()
                        .map(|p| format!("{p}/"))
                        .unwrap_or_default();
                    msg.push_str(&format!(
                        "\n  {}{}  ({})  {}:{}",
                        prefix, sym.name, sym.kind, sym.file, sym.start_line
                    ));
                }
            }
            return Err(msg);
        }
        // normalize-syntax-allow: rust/unwrap-in-impl - match arm guards exactly 1 match, so next() is always Some
        (1, 0) => matches.into_iter().next().unwrap(),
        (0, 1) => {
            let sym = &symbol_matches[0];
            return symbol::build_view_symbol_service(
                &sym.file,
                std::slice::from_ref(&sym.name),
                root,
                depth,
                false,
                docstring_mode,
                show_parent,
                context,
                case_insensitive,
            );
        }
        _ => {
            // Multiple matches → error with hint
            let mut text = format!(
                "Multiple matches for '{}' - be more specific, or use 'normalize view list':\n",
                target_str
            );
            for m in &matches {
                let kind = if m.is_directory { "directory" } else { "file" };
                text.push_str(&format!("  {} ({})\n", m.file_path, kind));
            }
            for sym in &symbol_matches {
                let sp = match &sym.parent {
                    Some(p) => format!("{}/{}", p, sym.name),
                    None => sym.name.clone(),
                };
                text.push_str(&format!(
                    "  {}/{} ({}, line {})\n",
                    sym.file, sp, sym.kind, sym.start_line
                ));
            }
            return Err(text);
        }
    };

    if unified.is_directory {
        tree::build_view_directory_service(
            &root.join(&unified.file_path),
            depth,
            raw,
            filter.as_ref(),
        )
    } else if full && unified.symbol_path.is_empty() {
        // --full: emit the raw source of the entire file via file service with depth out-of-range
        file::build_view_file_service(
            &unified.file_path,
            root,
            -1, // triggers full-source path
            _show_deps,
            types_only,
            show_tests,
            docstring_mode,
            context,
        )
    } else if unified.symbol_path.is_empty() {
        file::build_view_file_service(
            &unified.file_path,
            root,
            depth,
            _show_deps,
            types_only,
            show_tests,
            docstring_mode,
            context,
        )
    } else {
        // Check if symbol path contains glob patterns
        let symbol_pattern = unified.symbol_path.join("/");
        if path_resolve::is_glob_pattern(&symbol_pattern) {
            // Glob → multiple results; return error with suggestion
            let results =
                symbol::build_view_symbol_glob_service(&unified.file_path, &symbol_pattern, root)?;
            if results.len() == 1 {
                // normalize-syntax-allow: rust/unwrap-in-impl - len == 1 guarantees Some
                return Ok(results.into_iter().next().unwrap());
            }
            let mut msg = format!(
                "Pattern '{}' matched {} symbols - use 'normalize view list' to see all:\n",
                symbol_pattern,
                results.len()
            );
            for r in &results {
                msg.push_str(&format!("  {}\n", r.target));
            }
            return Err(msg);
        }

        symbol::build_view_symbol_service(
            &unified.file_path,
            &unified.symbol_path,
            root,
            depth,
            false,
            docstring_mode,
            show_parent,
            context,
            case_insensitive,
        )
    }
}

/// Build a list of view results matching the given filters.
#[allow(clippy::too_many_arguments)]
pub async fn build_view_list_service(
    target: Option<&str>,
    root: &Path,
    depth: i32,
    kind_filter: Option<&tree::SymbolKindFilter>,
    types_only: bool,
    show_tests: bool,
    raw: bool,
    _show_deps: bool,
    docstring_mode: DocstringDisplay,
    context: bool,
    show_parent: bool,
    exclude: &[String],
    only: &[String],
    case_insensitive: bool,
) -> Result<report::ViewListReport, String> {
    daemon::maybe_start_daemon(root);

    let filter = super::build_filter(root, exclude, only);

    // Kind filter → list matching symbols
    if let Some(kind) = kind_filter {
        let scope = target.unwrap_or(".");
        let results = tree::build_view_filtered_service(root, scope, kind)?;
        return Ok(report::ViewListReport(results));
    }

    let target_str = target.unwrap_or(".");

    // Directory → list children
    if target_str == "." {
        let dir_report = tree::build_view_directory_service(root, depth, raw, filter.as_ref())?;
        // Return all direct children as separate ViewReports
        let children: Vec<report::ViewReport> = dir_report
            .node
            .children
            .into_iter()
            .map(|child| {
                let tgt = child.path.clone();
                report::ViewReport {
                    target: tgt,
                    node: child,
                    source: None,
                    imports: Vec::new(),
                    exports: Vec::new(),
                    parent_signatures: Vec::new(),
                    line_range: None,
                    grammar: None,
                    warnings: Vec::new(),
                }
            })
            .collect();
        return Ok(report::ViewListReport(children));
    }

    // Symbol glob pattern in file
    let has_file_extension = target_str
        .rsplit('/')
        .next()
        .map(|last| last.contains('.'))
        .unwrap_or(false);

    let matches = path_resolve::resolve_unified_all(target_str, root);
    if !matches.is_empty() {
        let unified = &matches[0];
        if !unified.is_directory && !unified.symbol_path.is_empty() {
            let symbol_pattern = unified.symbol_path.join("/");
            if path_resolve::is_glob_pattern(&symbol_pattern) {
                let results = symbol::build_view_symbol_glob_service(
                    &unified.file_path,
                    &symbol_pattern,
                    root,
                )?;
                return Ok(report::ViewListReport(results));
            }
        }
    }

    // Symbol search → list all matches
    let symbol_matches = search::search_symbols(target_str, root).await;
    if !symbol_matches.is_empty() {
        let mut results = Vec::new();
        for sym in &symbol_matches {
            if let Ok(r) = symbol::build_view_symbol_service(
                &sym.file,
                std::slice::from_ref(&sym.name),
                root,
                depth,
                false,
                docstring_mode,
                show_parent,
                context,
                case_insensitive,
            ) {
                results.push(r);
            }
        }
        return Ok(report::ViewListReport(results));
    }

    // Path matches
    if !matches.is_empty() {
        let mut results = Vec::new();
        for m in matches {
            if m.is_directory {
                if let Ok(r) = tree::build_view_directory_service(
                    &root.join(&m.file_path),
                    depth,
                    raw,
                    filter.as_ref(),
                ) {
                    results.push(r);
                }
            } else if let Ok(r) = file::build_view_file_service(
                &m.file_path,
                root,
                depth,
                _show_deps,
                types_only,
                show_tests,
                docstring_mode,
                context,
            ) {
                results.push(r);
            }
        }
        return Ok(report::ViewListReport(results));
    }

    let _ = has_file_extension;
    Ok(report::ViewListReport(Vec::new()))
}
