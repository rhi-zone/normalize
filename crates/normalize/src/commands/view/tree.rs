//! Directory tree viewing for view command.

use super::report::{ViewKindFilterEntry, ViewKindFilterReport, ViewOutput};
use super::search::has_language_support;
use crate::filter::Filter;
use crate::output::OutputFormatter;
use crate::tree::{FormatOptions, ViewNode, ViewNodeKind};
use crate::{path_resolve, symbols, tree};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

/// Filter for symbol kinds in view command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKindFilter {
    Class,
    Function,
    Method,
}

impl FromStr for SymbolKindFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "class" | "classes" => Ok(SymbolKindFilter::Class),
            "function" | "functions" | "func" | "fn" => Ok(SymbolKindFilter::Function),
            "method" | "methods" => Ok(SymbolKindFilter::Method),
            _ => Err(format!(
                "invalid symbol kind '{}': expected 'class', 'function', or 'method'",
                s
            )),
        }
    }
}

impl fmt::Display for SymbolKindFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolKindFilter::Class => write!(f, "class"),
            SymbolKindFilter::Function => write!(f, "function"),
            SymbolKindFilter::Method => write!(f, "method"),
        }
    }
}

impl SymbolKindFilter {
    /// Returns the canonical string used for matching against symbol kinds.
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKindFilter::Class => "class",
            SymbolKindFilter::Function => "function",
            SymbolKindFilter::Method => "method",
        }
    }
}

/// Counts of files and directories in a tree.
struct NodeCounts {
    files: usize,
    dirs: usize,
}

/// View a directory as a tree
#[allow(clippy::too_many_arguments)]
pub fn cmd_view_directory(
    dir: &Path,
    _root: &Path,
    depth: i32,
    raw: bool,
    filter: Option<&Filter>,
) -> i32 {
    let json = false;
    let pretty = false;
    let use_colors = false;
    let effective_depth = if depth < 0 {
        None
    } else {
        Some(depth as usize)
    };

    let include_symbols = !(0..=1).contains(&depth);

    let view_node = tree::generate_view_tree(
        dir,
        &tree::TreeOptions {
            max_depth: effective_depth,
            collapse_single: !raw,
            include_symbols,
            ..Default::default()
        },
    );

    let view_node = if let Some(f) = filter {
        filter_view_node(view_node, f)
    } else {
        view_node
    };

    fn count_nodes(node: &ViewNode) -> NodeCounts {
        let mut counts = NodeCounts { files: 0, dirs: 0 };
        for child in &node.children {
            match child.kind {
                ViewNodeKind::Directory => {
                    counts.dirs += 1;
                    let sub = count_nodes(child);
                    counts.files += sub.files;
                    counts.dirs += sub.dirs;
                }
                ViewNodeKind::File => counts.files += 1,
                ViewNodeKind::Symbol(_) => {}
            }
        }
        counts
    }
    let counts = count_nodes(&view_node);
    let (file_count, dir_count) = (counts.files, counts.dirs);

    if json {
        let report = ViewOutput::Directory { node: view_node };
        println!("{}", report.format_text());
    } else {
        let format_options = FormatOptions {
            minimal: !pretty,
            use_colors,
            ..Default::default()
        };
        let lines = tree::format_view_node(&view_node, &format_options);
        for line in &lines {
            println!("{}", line);
        }
        println!();
        println!("{} directories, {} files", dir_count, file_count);
    }
    0
}

/// Build a directory tree view for the service layer.
pub fn build_view_directory_service(
    dir: &Path,
    depth: i32,
    raw: bool,
    filter: Option<&Filter>,
) -> Result<ViewOutput, String> {
    let effective_depth = if depth < 0 {
        None
    } else {
        Some(depth as usize)
    };

    let include_symbols = !(0..=1).contains(&depth);

    let view_node = tree::generate_view_tree(
        dir,
        &tree::TreeOptions {
            max_depth: effective_depth,
            collapse_single: !raw,
            include_symbols,
            ..Default::default()
        },
    );

    let view_node = if let Some(f) = filter {
        filter_view_node(view_node, f)
    } else {
        view_node
    };

    Ok(ViewOutput::Directory { node: view_node })
}

/// Filter a ViewNode tree, removing nodes that don't pass the filter.
fn filter_view_node(mut node: ViewNode, filter: &Filter) -> ViewNode {
    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let path = std::path::Path::new(&child.path);
            match child.kind {
                ViewNodeKind::Directory => {
                    let filtered = filter_view_node(child, filter);
                    if filtered.children.is_empty() {
                        None
                    } else {
                        Some(filtered)
                    }
                }
                ViewNodeKind::File => {
                    if filter.matches(path) {
                        Some(child)
                    } else {
                        None
                    }
                }
                ViewNodeKind::Symbol(_) => Some(child),
            }
        })
        .collect();
    node
}

/// List symbols matching a kind filter within a scope
pub fn cmd_view_filtered(root: &Path, scope: &str, kind: &str) -> i32 {
    let json = false;
    let kind_filter: Option<SymbolKindFilter> = match kind.to_lowercase().as_str() {
        "all" | "*" => None,
        other => match other.parse::<SymbolKindFilter>() {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        },
    };

    let files_to_search: Vec<std::path::PathBuf> = if scope == "." {
        path_resolve::all_files(root)
            .into_iter()
            .filter(|m| m.kind == "file" && has_language_support(&m.path))
            .map(|m| root.join(&m.path))
            .collect()
    } else {
        let matches = path_resolve::resolve(scope, root);
        matches
            .into_iter()
            .filter(|m| m.kind == "file" && has_language_support(&m.path))
            .map(|m| root.join(&m.path))
            .collect()
    };

    let mut all_symbols: Vec<(String, String, String, usize, Option<String>)> = Vec::new();
    let parser = symbols::SymbolParser::new();

    for file_path in files_to_search {
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = file_path
            .strip_prefix(root)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();

        let syms = parser.parse_file(&file_path, &content);
        for sym in syms {
            let sym_kind = sym.kind.as_str();
            if let Some(ref filter) = kind_filter
                && sym_kind != filter.as_str()
            {
                continue;
            }
            all_symbols.push((
                rel_path.clone(),
                sym.name,
                sym_kind.to_string(),
                sym.start_line,
                sym.parent,
            ));
        }
    }

    if all_symbols.is_empty() {
        if json {
            println!("[]");
        } else {
            eprintln!("No symbols found matching type: {}", kind);
        }
        return 1;
    }

    all_symbols.sort_by(|a, b| (&a.0, a.3).cmp(&(&b.0, b.3)));

    if json {
        let report = ViewOutput::KindFilter(ViewKindFilterReport {
            symbols: all_symbols
                .iter()
                .map(|(file, name, kind, line, parent)| ViewKindFilterEntry {
                    file: file.clone(),
                    name: name.clone(),
                    kind: kind.clone(),
                    line: *line,
                    parent: parent.clone(),
                })
                .collect(),
        });
        println!("{}", report.format_text());
    } else {
        for (file, name, kind, line, parent) in &all_symbols {
            let parent_str = parent
                .as_ref()
                .map(|p| format!(" (in {})", p))
                .unwrap_or_default();
            println!("{}:{} {} {}{}", file, line, kind, name, parent_str);
        }
        eprintln!("\n{} symbols found", all_symbols.len());
    }

    0
}

/// Build kind-filtered symbols for the service layer.
pub fn build_view_filtered_service(
    root: &Path,
    scope: &str,
    kind: &SymbolKindFilter,
) -> Result<ViewOutput, String> {
    let files_to_search: Vec<std::path::PathBuf> = if scope == "." {
        path_resolve::all_files(root)
            .into_iter()
            .filter(|m| m.kind == "file" && has_language_support(&m.path))
            .map(|m| root.join(&m.path))
            .collect()
    } else {
        let matches = path_resolve::resolve(scope, root);
        matches
            .into_iter()
            .filter(|m| m.kind == "file" && has_language_support(&m.path))
            .map(|m| root.join(&m.path))
            .collect()
    };

    let mut all_symbols: Vec<(String, String, String, usize, Option<String>)> = Vec::new();
    let parser = symbols::SymbolParser::new();

    for file_path in files_to_search {
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = file_path
            .strip_prefix(root)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();

        let syms = parser.parse_file(&file_path, &content);
        for sym in syms {
            let sym_kind = sym.kind.as_str();
            if sym_kind != kind.as_str() {
                continue;
            }
            all_symbols.push((
                rel_path.clone(),
                sym.name,
                sym_kind.to_string(),
                sym.start_line,
                sym.parent,
            ));
        }
    }

    if all_symbols.is_empty() {
        return Err(format!("No symbols found matching type: {}", kind));
    }

    all_symbols.sort_by(|a, b| (&a.0, a.3).cmp(&(&b.0, b.3)));

    Ok(ViewOutput::KindFilter(ViewKindFilterReport {
        symbols: all_symbols
            .iter()
            .map(|(file, name, kind_s, line, parent)| ViewKindFilterEntry {
                file: file.clone(),
                name: name.clone(),
                kind: kind_s.clone(),
                line: *line,
                parent: parent.clone(),
            })
            .collect(),
    }))
}
