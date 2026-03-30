//! Complexity delta diff metric.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops;
use normalize_languages::{Language, support_for_path};
use std::collections::HashMap;
use std::path::Path;

/// Complexity increase/decrease per function.
///
/// Compares complexity output at `base_ref` vs the working tree.
/// Returns measurements with `(file/Parent/fn, increase, decrease)` for each function
/// whose complexity changed.
pub struct ComplexityDeltaMetric;

impl DiffMetric for ComplexityDeltaMetric {
    fn name(&self) -> &'static str {
        "complexity-delta"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let base_map = collect_complexity_at_ref(root, base_ref)?;
        let current_map = collect_complexity_from_disk(root);

        let mut results = Vec::new();
        for (key, current_val) in &current_map {
            if let Some(&base_val) = base_map.get(key) {
                let delta = *current_val - base_val;
                if delta > 0.0 {
                    results.push(DiffMeasurement {
                        key: key.clone(),
                        added: delta,
                        removed: 0.0,
                    });
                } else if delta < 0.0 {
                    results.push(DiffMeasurement {
                        key: key.clone(),
                        added: 0.0,
                        removed: -delta,
                    });
                }
            } else {
                // New function — count its full complexity as added
                results.push(DiffMeasurement {
                    key: key.clone(),
                    added: *current_val,
                    removed: 0.0,
                });
            }
        }
        // Functions removed from base
        for (key, base_val) in &base_map {
            if !current_map.contains_key(key) {
                results.push(DiffMeasurement {
                    key: key.clone(),
                    added: 0.0,
                    removed: *base_val,
                });
            }
        }

        Ok(results)
    }
}

/// Collect complexity metrics from all files in the git tree at `git_ref`.
///
/// Uses gix to read file blobs directly from the object store — no filesystem checkout.
fn collect_complexity_at_ref(root: &Path, git_ref: &str) -> anyhow::Result<HashMap<String, f64>> {
    let repo = git_ops::open_repo(root)?;
    let mut map = HashMap::new();

    git_ops::walk_tree_at_ref(root, git_ref, |rel_path, blob_id| {
        let abs_path = root.join(rel_path);
        let Some(support) = support_for_path(&abs_path) else {
            return;
        };
        let Some(content) = git_ops::read_blob_text(&repo, blob_id) else {
            return;
        };
        if let Some(entries) = analyze_file_complexity(&abs_path, rel_path, &content, support) {
            map.extend(entries);
        }
    })?;

    Ok(map)
}

/// Collect complexity metrics from the current working tree (files on disk).
fn collect_complexity_from_disk(root: &Path) -> HashMap<String, f64> {
    let all_files = normalize_path_resolve::all_files(root, None);
    let mut map = HashMap::new();

    for entry in &all_files {
        if entry.kind != normalize_path_resolve::PathMatchKind::File {
            continue;
        }
        let abs_path = root.join(&entry.path);
        let Some(support) = support_for_path(&abs_path) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&abs_path) else {
            continue;
        };

        let rel = entry.path.replace('\\', "/");
        if let Some(entries) = analyze_file_complexity(&abs_path, &rel, &content, support) {
            map.extend(entries);
        }
    }
    map
}

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
    let ts_lang = loader.get(grammar_name).ok()?;
    let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;

    let complexity_query = loader.get_complexity(grammar_name).and_then(|scm| {
        let grammar = loader.get(grammar_name).ok()?;
        tree_sitter::Query::new(&grammar, &scm).ok()
    });

    struct TagInfo {
        start_byte: usize,
        end_byte: usize,
        start_row: usize,
        end_row: usize,
        is_container: bool,
    }

    let root_node = tree.root_node();
    let tag_infos: Vec<TagInfo> = {
        use streaming_iterator::StreamingIterator;
        let capture_names = tags_query.capture_names();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());
        let mut infos: Vec<TagInfo> = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let Some(cn) = capture_names.get(capture.index as usize) else {
                    continue;
                };
                let is_fn = matches!(*cn, "definition.function" | "definition.method");
                let is_container = matches!(
                    *cn,
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

    let mut tag_infos = tag_infos;
    tag_infos.sort_by(|a, b| {
        a.start_row
            .cmp(&b.start_row)
            .then(b.end_row.cmp(&a.end_row))
    });
    tag_infos.dedup_by(|b, a| a.start_byte == b.start_byte && a.end_byte == b.end_byte);

    let mut results = Vec::new();

    for i in 0..tag_infos.len() {
        if tag_infos[i].is_container {
            continue;
        }
        let ti = &tag_infos[i];

        // Use `let Some` instead of `?` so a single missing node skips this function
        // rather than aborting measurement for the entire file.
        let Some(fn_node) = find_node_by_range(root_node, ti.start_byte, ti.end_byte) else {
            continue;
        };

        let parent_name: Option<String> = tag_infos
            .iter()
            .enumerate()
            .filter(|(j, c)| *j != i && c.is_container)
            .filter(|(_, c)| c.start_row <= ti.start_row && c.end_row >= ti.end_row)
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
            normalize_facts::extract::compute_complexity(&fn_node, support, content.as_bytes())
        };

        let address = if let Some(parent) = &parent_name {
            format!("{rel_path}/{parent}/{name}")
        } else {
            format!("{rel_path}/{name}")
        };
        let address = address.replace('\\', "/");
        let _ = abs_path;
        results.push((address, complexity as f64));
    }

    Some(results)
}

fn count_complexity_with_query(
    node: &tree_sitter::Node,
    query: &tree_sitter::Query,
    content: &str,
) -> usize {
    use streaming_iterator::StreamingIterator;
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(query, *node, content.as_bytes());
    let mut count = 1usize;
    while let Some(m) = matches.next() {
        count = count.saturating_add(m.captures.len());
    }
    count
}

const MAX_DEPTH: usize = 512;

fn find_node_by_range(
    node: tree_sitter::Node,
    start_byte: usize,
    end_byte: usize,
) -> Option<tree_sitter::Node> {
    find_node_by_range_inner(node, start_byte, end_byte, 0)
}

fn find_node_by_range_inner(
    node: tree_sitter::Node,
    start_byte: usize,
    end_byte: usize,
    depth: usize,
) -> Option<tree_sitter::Node> {
    if depth > MAX_DEPTH {
        return None;
    }
    if node.start_byte() == start_byte && node.end_byte() == end_byte {
        return Some(node);
    }
    if node.start_byte() > start_byte || node.end_byte() < end_byte {
        return None;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(found) =
                find_node_by_range_inner(cursor.node(), start_byte, end_byte, depth + 1)
            {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}
