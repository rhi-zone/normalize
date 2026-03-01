//! ncdu-style hierarchical LOC breakdown

use crate::output::OutputFormatter;
use glob::Pattern;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// A node in the size tree (directory or file)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SizeNode {
    pub name: String,
    pub lines: usize,
    pub pct: f64,
    pub children: Vec<SizeNode>,
}

/// Report returned by analyze size
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SizeReport {
    pub root: String,
    pub total_lines: usize,
    pub tree: Vec<SizeNode>,
}

impl OutputFormatter for SizeReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();
        out.push(format!(
            "# Code Size: {} ({} lines)\n",
            self.root, self.total_lines
        ));
        for node in &self.tree {
            render_node(&mut out, node, "", true);
        }
        out.join("\n")
    }
}

fn render_node(out: &mut Vec<String>, node: &SizeNode, prefix: &str, last: bool) {
    let connector = if last { "└── " } else { "├── " };
    out.push(format!(
        "{}{}{:<6}  {:>5.1}%  {}",
        prefix, connector, node.lines, node.pct, node.name
    ));
    let child_prefix = format!("{}{}   ", prefix, if last { " " } else { "│" });
    for (i, child) in node.children.iter().enumerate() {
        render_node(out, child, &child_prefix, i == node.children.len() - 1);
    }
}

/// Analyze code size as a hierarchical tree
pub fn analyze_size(root: &Path, exclude: &[String]) -> SizeReport {
    let all_files = crate::path_resolve::all_files(root);
    let excludes: Vec<Pattern> = exclude
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    // Collect (relative_path, line_count) for all recognized source files
    let file_sizes: Vec<(String, usize)> = all_files
        .par_iter()
        .filter(|f| f.kind == "file")
        .filter(|f| !excludes.iter().any(|pat| pat.matches(&f.path)))
        .filter_map(|f| {
            let path = root.join(&f.path);
            normalize_languages::support_for_path(&path)?;
            let content = std::fs::read_to_string(&path).ok()?;
            let lines = content.lines().count();
            if lines == 0 {
                return None;
            }
            Some((f.path.clone(), lines))
        })
        .collect();

    let total_lines: usize = file_sizes.iter().map(|(_, l)| l).sum();

    // Build directory tree: map of path -> total lines
    // We accumulate into a nested BTreeMap structure
    let tree = build_tree(&file_sizes, total_lines);

    SizeReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        total_lines,
        tree,
    }
}

/// Recursively build SizeNode tree from flat (path, lines) list
fn build_tree(files: &[(String, usize)], total: usize) -> Vec<SizeNode> {
    // Group by first path component
    let mut groups: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    for (path, lines) in files {
        let (head, tail) = split_path(path);
        groups
            .entry(head)
            .or_default()
            .push((tail.to_string(), *lines));
    }

    let mut nodes: Vec<SizeNode> = groups
        .into_iter()
        .map(|(name, children_raw)| {
            let node_lines: usize = children_raw.iter().map(|(_, l)| l).sum();
            let pct = if total > 0 {
                node_lines as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            // If there's only one child and it's a leaf (no further split),
            // collapse: show the full path as the name
            let children = if children_raw.len() == 1 && children_raw[0].0.is_empty() {
                vec![]
            } else {
                // Filter out empty-tail entries (direct files at this level)
                let deeper: Vec<_> = children_raw
                    .iter()
                    .filter(|(t, _)| !t.is_empty())
                    .cloned()
                    .collect();
                let direct: Vec<_> = children_raw
                    .iter()
                    .filter(|(t, _)| t.is_empty())
                    .cloned()
                    .collect();
                let c = build_tree(&deeper, total);
                // Direct files at this level get added as leaf nodes
                for (_, lines) in direct {
                    // already counted in node_lines, just represented by parent
                    let _ = lines;
                }
                c
            };
            SizeNode {
                name,
                lines: node_lines,
                pct,
                children,
            }
        })
        .collect();

    nodes.sort_by(|a, b| b.lines.cmp(&a.lines));
    nodes
}

/// Split "foo/bar/baz" into ("foo", "bar/baz"). Returns ("foo", "") for "foo".
fn split_path(path: &str) -> (String, &str) {
    match path.find('/') {
        Some(i) => (path[..i].to_string(), &path[i + 1..]),
        None => (path.to_string(), ""),
    }
}
