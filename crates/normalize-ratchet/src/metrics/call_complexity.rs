//! Transitive (call-graph BFS) cyclomatic complexity metric.

use super::Metric;
use normalize_languages::support_for_path;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// Transitive/effective cyclomatic complexity via call-graph BFS.
///
/// Returns `(file/Parent/fn, reachable_cc as f64)` for every function.
pub struct CallComplexityMetric;

type FnKey = (String, String); // (rel_path, fn_symbol)

impl Metric for CallComplexityMetric {
    fn name(&self) -> &'static str {
        "call-complexity"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let loader = normalize_facts::grammar_loader();

        type FileResult = (
            Vec<(FnKey, usize, Option<String>)>, // (key, local_cc, parent)
            Vec<(FnKey, String)>,                // (caller_key, callee_name)
        );

        let per_file: Vec<FileResult> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                let support = support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                if content.is_empty() {
                    return None;
                }
                let grammar_name = support.grammar_name();
                let tree = normalize_facts::parse_with_grammar(grammar_name, &content)?;
                let tags_scm = loader.get_tags(grammar_name)?;
                let ts_lang = loader.get(grammar_name).ok()?;
                let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;
                let complexity_query = loader.get_complexity(grammar_name).and_then(|scm| {
                    let grammar = loader.get(grammar_name).ok()?;
                    tree_sitter::Query::new(&grammar, &scm).ok()
                });
                let calls_query = loader.get_calls(grammar_name).and_then(|scm| {
                    let grammar = loader.get(grammar_name).ok()?;
                    tree_sitter::Query::new(&grammar, &scm).ok()
                });

                let root_node = tree.root_node();

                // Collect tag info: (start_byte, end_byte, start_row, end_row, is_container, name)
                struct TagInfo {
                    start_byte: usize,
                    end_byte: usize,
                    start_row: usize,
                    end_row: usize,
                    is_container: bool,
                }

                let capture_names = tags_query.capture_names();
                let mut tag_infos: Vec<TagInfo> = {
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

                // Sort and dedup
                tag_infos.sort_by(|a, b| {
                    a.start_row
                        .cmp(&b.start_row)
                        .then(b.end_row.cmp(&a.end_row))
                });
                tag_infos.dedup_by(|b, a| a.start_byte == b.start_byte && a.end_byte == b.end_byte);

                // Build fn_entries and a parallel row-range list in lockstep so that
                // fn_row_ranges[i] always corresponds to fn_entries[i].  We must only
                // add to both when node_name() succeeds; a `continue` that skips only
                // one of them would misalign subsequent indices.
                struct FnRowRange {
                    start_row: usize,
                    end_row: usize,
                }
                let mut fn_entries: Vec<(FnKey, usize, Option<String>)> = Vec::new();
                let mut fn_row_ranges_raw: Vec<FnRowRange> = Vec::new();

                for i in 0..tag_infos.len() {
                    if tag_infos[i].is_container {
                        continue;
                    }
                    let ti = &tag_infos[i];
                    let fn_start_row = ti.start_row;
                    let fn_end_row = ti.end_row;

                    // Find the tree node for this function (by byte range)
                    let fn_node = match find_node_by_range(root_node, ti.start_byte, ti.end_byte) {
                        Some(n) => n,
                        None => continue,
                    };

                    // Find parent container
                    let parent_name: Option<String> = tag_infos
                        .iter()
                        .enumerate()
                        .filter(|(j, c)| *j != i && c.is_container)
                        .filter(|(_, c)| c.start_row <= fn_start_row && c.end_row >= fn_end_row)
                        .max_by_key(|(_, c)| c.start_row)
                        .and_then(|(_, c)| {
                            find_node_by_range(root_node, c.start_byte, c.end_byte).and_then(|n| {
                                support.node_name(&n, &content).map(|s| s.to_string())
                            })
                        });

                    let name = match support.node_name(&fn_node, &content) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };

                    let local_cc = if let Some(ref cq) = complexity_query {
                        count_with_query(&fn_node, cq, &content)
                    } else {
                        normalize_facts::extract::compute_complexity(
                            &fn_node,
                            support,
                            content.as_bytes(),
                        )
                    };

                    let key = (f.path.clone(), name.clone());
                    fn_entries.push((key, local_cc, parent_name));
                    fn_row_ranges_raw.push(FnRowRange {
                        start_row: fn_start_row,
                        end_row: fn_end_row,
                    });
                }

                // Build the row-range lookup used for call-edge containment.
                let fn_row_ranges: Vec<(usize, usize, FnKey)> = fn_entries
                    .iter()
                    .zip(fn_row_ranges_raw.iter())
                    .map(|((key, _, _), rr)| (rr.start_row, rr.end_row, key.clone()))
                    .collect();

                let mut call_edges: Vec<(FnKey, String)> = Vec::new();
                if let Some(ref cq) = calls_query {
                    let cq_capture_names = cq.capture_names();
                    let mut qcursor2 = tree_sitter::QueryCursor::new();
                    let mut call_matches = qcursor2.matches(cq, root_node, content.as_bytes());
                    while let Some(m) = call_matches.next() {
                        for cap in m.captures {
                            let cn = cq_capture_names[cap.index as usize];
                            if cn == "reference.call" {
                                let callee_name = content[cap.node.byte_range()].to_string();
                                let call_row = cap.node.start_position().row;
                                // Find innermost function enclosing this call by row
                                let caller_key = fn_row_ranges
                                    .iter()
                                    .filter(|(sr, er, _)| call_row >= *sr && call_row <= *er)
                                    .max_by_key(|(sr, _, _)| *sr)
                                    .map(|(_, _, key)| key);
                                if let Some(caller_key) = caller_key {
                                    call_edges.push((caller_key.clone(), callee_name));
                                }
                            }
                        }
                    }
                }

                Some((fn_entries, call_edges))
            })
            .collect();

        // Collect all cc data and raw edges
        let mut cc_map: HashMap<FnKey, usize> = HashMap::new();
        let mut fn_parents: HashMap<FnKey, Option<String>> = HashMap::new();
        let mut all_raw_edges: Vec<(FnKey, String)> = Vec::new();
        for (fn_entries, edges) in per_file {
            for (key, cc, parent) in fn_entries {
                cc_map.insert(key.clone(), cc);
                fn_parents.insert(key, parent);
            }
            all_raw_edges.extend(edges);
        }

        // Build name-to-keys index
        let mut name_index: HashMap<String, Vec<FnKey>> = HashMap::new();
        for key in cc_map.keys() {
            name_index
                .entry(key.1.clone())
                .or_default()
                .push(key.clone());
        }

        // Resolve call edges
        let mut call_graph: HashMap<FnKey, Vec<FnKey>> = HashMap::new();
        for (caller_key, callee_name) in all_raw_edges {
            let name = callee_name
                .trim_end_matches("()")
                .trim_start_matches("self::")
                .trim_start_matches("Self::");
            if let Some(candidates) = name_index.get(name) {
                let target = candidates
                    .iter()
                    .find(|(f, _)| *f == caller_key.0)
                    .or_else(|| candidates.first())
                    .cloned();
                if let Some(t) = target {
                    call_graph.entry(caller_key).or_default().push(t);
                }
            }
        }
        // Dedup adjacency lists to prevent inflated BFS sums from duplicate call sites.
        for callees in call_graph.values_mut() {
            callees.sort_unstable();
            callees.dedup();
        }

        // BFS from each function to compute reachable CC sum
        let results: Vec<(String, f64)> = cc_map
            .par_iter()
            .map(|(key, &local_cc)| {
                let reachable = bfs_reachable(key, &cc_map, &call_graph);
                let parent = fn_parents.get(key).and_then(|p| p.clone());
                let address = if let Some(p) = parent {
                    format!("{}/{}/{}", key.0, p, key.1)
                } else {
                    format!("{}/{}", key.0, key.1)
                };
                (address.replace('\\', "/"), (local_cc + reachable) as f64)
            })
            .collect();

        Ok(results)
    }
}

/// Find a node by exact byte range (depth-first).
fn find_node_by_range(
    node: tree_sitter::Node,
    start_byte: usize,
    end_byte: usize,
) -> Option<tree_sitter::Node> {
    if node.start_byte() == start_byte && node.end_byte() == end_byte {
        return Some(node);
    }
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

/// BFS to compute total reachable CC (excluding start node's local CC).
fn bfs_reachable(
    start: &FnKey,
    cc_map: &HashMap<FnKey, usize>,
    call_graph: &HashMap<FnKey, Vec<FnKey>>,
) -> usize {
    let mut visited: HashSet<&FnKey> = HashSet::new();
    let mut queue: VecDeque<&FnKey> = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);
    let mut total = 0usize;
    while let Some(current) = queue.pop_front() {
        if let Some(callees) = call_graph.get(current) {
            for callee in callees {
                if visited.insert(callee) {
                    total += cc_map.get(callee).copied().unwrap_or(1);
                    queue.push_back(callee);
                }
            }
        }
    }
    total
}

fn count_with_query(node: &tree_sitter::Node, query: &tree_sitter::Query, content: &str) -> usize {
    use streaming_iterator::StreamingIterator;
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(query, *node, content.as_bytes());
    let mut count = 1usize;
    while let Some(m) = matches.next() {
        count += m.captures.len();
    }
    count
}
