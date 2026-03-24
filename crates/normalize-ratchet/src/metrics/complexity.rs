//! Cyclomatic complexity metric using normalize-facts + normalize-languages.

use super::Metric;
use normalize_facts::extract::compute_complexity;
use normalize_languages::{Language, support_for_path};
use rayon::prelude::*;
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// Cyclomatic complexity per function.
///
/// Returns `(file/Parent/fn, complexity as f64)` for every function.
pub struct ComplexityMetric;

impl Metric for ComplexityMetric {
    fn name(&self) -> &'static str {
        "complexity"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let results: Vec<Vec<(String, f64)>> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                let support = support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let entries = analyze_file_complexity(&abs_path, &f.path, &content, support)?;
                Some(entries)
            })
            .collect();
        Ok(results.into_iter().flatten().collect())
    }
}

/// Analyze complexity of a single file, returning `(address, complexity)` pairs.
fn analyze_file_complexity(
    abs_path: &Path,
    rel_path: &str,
    content: &str,
    support: &dyn Language,
) -> Option<Vec<(String, f64)>> {
    let grammar_name = support.grammar_name();
    let loader = normalize_facts::grammar_loader();
    let tree = normalize_facts::parse_with_grammar(grammar_name, content)?;

    let tags_scm = loader.get_tags(grammar_name)?;
    let ts_lang = loader.get(grammar_name).ok().flatten()?;
    let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;

    let complexity_query = loader.get_complexity(grammar_name).and_then(|scm| {
        let grammar = loader.get(grammar_name).ok().flatten()?;
        tree_sitter::Query::new(&grammar, &scm).ok()
    });

    // Collect tag info without holding borrows across the mutable borrow of qcursor
    struct TagInfo {
        start_byte: usize,
        end_byte: usize,
        start_row: usize,
        end_row: usize,
        is_container: bool,
    }

    let root_node = tree.root_node();
    let tag_infos: Vec<TagInfo> = {
        let capture_names = tags_query.capture_names();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());
        let mut infos: Vec<TagInfo> = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let cn = capture_names[capture.index as usize];
                let is_fn = matches!(cn, "definition.function" | "definition.method");
                let is_container = matches!(
                    cn,
                    "definition.class" | "definition.module" | "definition.interface"
                );
                if is_fn || is_container {
                    infos.push(TagInfo {
                        start_byte: capture.node.start_byte(),
                        end_byte: capture.node.end_byte(),
                        start_row: capture.node.start_position().row,
                        end_row: capture.node.end_position().row,
                        is_container,
                    });
                }
            }
        }
        infos
    };

    // Sort by start_row (containers first on tie)
    let mut tag_infos = tag_infos;
    tag_infos.sort_by(|a, b| {
        a.start_row
            .cmp(&b.start_row)
            .then(b.end_row.cmp(&a.end_row))
    });
    // Dedup identical byte ranges
    tag_infos.dedup_by(|b, a| a.start_byte == b.start_byte && a.end_byte == b.end_byte);

    let mut results = Vec::new();

    for i in 0..tag_infos.len() {
        if tag_infos[i].is_container {
            continue;
        }
        let ti = &tag_infos[i];
        let fn_start_row = ti.start_row;
        let fn_end_row = ti.end_row;

        // Find the actual tree node by byte range — skip this function if not found,
        // do not abort the whole file with `?`.
        let fn_node = match find_node_by_range(root_node, ti.start_byte, ti.end_byte) {
            Some(n) => n,
            None => continue,
        };

        // Find innermost enclosing container
        let parent_name: Option<String> = tag_infos
            .iter()
            .enumerate()
            .filter(|(j, c)| *j != i && c.is_container)
            .filter(|(_, c)| c.start_row <= fn_start_row && c.end_row >= fn_end_row)
            .max_by_key(|(_, c)| c.start_row)
            .and_then(|(_, c)| {
                find_node_by_range(root_node, c.start_byte, c.end_byte)
                    .and_then(|n| support.node_name(&n, content).map(|s| s.to_string()))
            });

        let name = match support.node_name(&fn_node, content) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let complexity = if let Some(ref cq) = complexity_query {
            count_complexity_with_query(&fn_node, cq, content)
        } else {
            compute_complexity(&fn_node, support, content.as_bytes())
        };

        let address = if let Some(parent) = &parent_name {
            format!("{rel_path}/{parent}/{name}")
        } else {
            format!("{rel_path}/{name}")
        };
        let address = address.replace('\\', "/");
        let _ = abs_path; // unused after normalization
        results.push((address, complexity as f64));
    }

    Some(results)
}

/// Find a tree node with exact byte range (depth-first search).
fn find_node_by_range(
    node: tree_sitter::Node,
    start_byte: usize,
    end_byte: usize,
) -> Option<tree_sitter::Node> {
    if node.start_byte() == start_byte && node.end_byte() == end_byte {
        return Some(node);
    }
    // Only recurse into nodes that could contain our target
    if node.start_byte() > start_byte || node.end_byte() < end_byte {
        return None;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(found) = find_node_by_range(cursor.node(), start_byte, end_byte) {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Count complexity using a `@complexity` query.
fn count_complexity_with_query(
    node: &tree_sitter::Node,
    query: &tree_sitter::Query,
    content: &str,
) -> usize {
    use streaming_iterator::StreamingIterator;
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(query, *node, content.as_bytes());
    let mut count = 1usize; // base complexity
    while let Some(m) = matches.next() {
        count += m.captures.len();
    }
    count
}
