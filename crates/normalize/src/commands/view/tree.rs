//! Directory tree viewing for view command.

use super::report::{ViewKindFilterEntry, ViewKindFilterReport, ViewOutput, ViewResult};
use super::search::has_language_support;
use crate::filter::Filter;
use crate::output::OutputFormatter;
use crate::tree::{FormatOptions, ViewNode, ViewNodeKind};
use crate::{path_resolve, symbols, tree};
use std::path::Path;

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
    format: &crate::output::OutputFormat,
    filter: Option<&Filter>,
) -> i32 {
    let json = format.is_json();
    let pretty = format.is_pretty();
    let use_colors = format.use_colors();
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
        report.print(format);
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
/// Returns ViewResult containing the ViewOutput and rendered text.
pub fn build_view_directory_service(
    dir: &Path,
    depth: i32,
    raw: bool,
    filter: Option<&Filter>,
    use_colors: bool,
) -> Result<ViewResult, String> {
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

    let (dir_count, file_count) = {
        let mut dirs = 0usize;
        let mut files = 0usize;
        fn count(node: &ViewNode, dirs: &mut usize, files: &mut usize) {
            for child in &node.children {
                match child.kind {
                    ViewNodeKind::Directory => {
                        *dirs += 1;
                        count(child, dirs, files);
                    }
                    ViewNodeKind::File => *files += 1,
                    ViewNodeKind::Symbol(_) => {}
                }
            }
        }
        count(&view_node, &mut dirs, &mut files);
        (dirs, files)
    };

    let format_options = FormatOptions {
        minimal: !use_colors,
        use_colors,
        ..Default::default()
    };
    let lines = tree::format_view_node(&view_node, &format_options);
    let mut text = lines.join("\n");
    text.push('\n');
    text.push('\n');
    text.push_str(&format!("{} directories, {} files", dir_count, file_count));

    Ok(ViewResult {
        output: ViewOutput::Directory { node: view_node },
        text,
    })
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
pub fn cmd_view_filtered(
    root: &Path,
    scope: &str,
    kind: &str,
    format: &crate::output::OutputFormat,
) -> i32 {
    let json = format.is_json();
    let kind_lower = kind.to_lowercase();
    let kind_filter = match kind_lower.as_str() {
        "class" | "classes" => Some("class"),
        "function" | "functions" | "func" | "fn" => Some("function"),
        "method" | "methods" => Some("method"),
        "all" | "*" => None,
        _ => {
            eprintln!(
                "Unknown type: {}. Valid types: class, function, method",
                kind
            );
            return 1;
        }
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
            if let Some(filter) = kind_filter
                && sym_kind != filter
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
        report.print(format);
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
    kind: &str,
) -> Result<ViewResult, String> {
    let kind_lower = kind.to_lowercase();
    let kind_filter: Option<&str> = match kind_lower.as_str() {
        "class" | "classes" => Some("class"),
        "function" | "functions" | "func" | "fn" => Some("function"),
        "method" | "methods" => Some("method"),
        "all" | "*" => None,
        _ => {
            return Err(format!(
                "Unknown type: {}. Valid types: class, function, method",
                kind
            ));
        }
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
            if let Some(kf) = kind_filter
                && sym_kind != kf
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
        return Err(format!("No symbols found matching type: {}", kind));
    }

    all_symbols.sort_by(|a, b| (&a.0, a.3).cmp(&(&b.0, b.3)));

    let mut text = String::new();
    for (file, name, kind_s, line, parent) in &all_symbols {
        let parent_str = parent
            .as_ref()
            .map(|p| format!(" (in {})", p))
            .unwrap_or_default();
        text.push_str(&format!(
            "{}:{} {} {}{}\n",
            file, line, kind_s, name, parent_str
        ));
    }
    text.push_str(&format!("\n{} symbols found", all_symbols.len()));

    let output = ViewOutput::KindFilter(ViewKindFilterReport {
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
    });

    Ok(ViewResult { output, text })
}
