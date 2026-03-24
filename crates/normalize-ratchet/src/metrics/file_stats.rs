//! File-level stats metrics: line count, function count, class count, comment line count.

use super::Metric;
use normalize_languages::support_for_path;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// Lines per file.
pub struct LineCountMetric;

/// Number of functions/methods per file.
pub struct FunctionCountMetric;

/// Number of classes/structs/types per file.
pub struct ClassCountMetric;

/// Number of comment lines per file.
pub struct CommentLineCountMetric;

impl Metric for LineCountMetric {
    fn name(&self) -> &'static str {
        "line-count"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let results: Vec<(String, f64)> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                // Only count code files
                support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let count = content.lines().count();
                Some((f.path.replace('\\', "/"), count as f64))
            })
            .collect();
        Ok(results)
    }
}

impl Metric for FunctionCountMetric {
    fn name(&self) -> &'static str {
        "function-count"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let loader = normalize_facts::grammar_loader();
        let results: Vec<(String, f64)> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                let support = support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let grammar_name = support.grammar_name();
                let tree = normalize_facts::parse_with_grammar(grammar_name, &content)?;
                let tags_scm = loader.get_tags(grammar_name)?;
                let ts_lang = loader.get(grammar_name)?;
                let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;

                let capture_names = tags_query.capture_names();
                let root_node = tree.root_node();
                let mut qcursor = tree_sitter::QueryCursor::new();
                let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());

                let mut count = 0usize;
                let mut seen: HashSet<(usize, usize)> = HashSet::new();
                while let Some(m) = matches.next() {
                    for capture in m.captures {
                        let cn = capture_names[capture.index as usize];
                        if matches!(cn, "definition.function" | "definition.method") {
                            let key = (capture.node.start_byte(), capture.node.end_byte());
                            if seen.insert(key) {
                                count += 1;
                            }
                        }
                    }
                }
                Some((f.path.replace('\\', "/"), count as f64))
            })
            .collect();
        Ok(results)
    }
}

impl Metric for ClassCountMetric {
    fn name(&self) -> &'static str {
        "class-count"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let loader = normalize_facts::grammar_loader();
        let results: Vec<(String, f64)> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                let support = support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let grammar_name = support.grammar_name();
                let tree = normalize_facts::parse_with_grammar(grammar_name, &content)?;
                let tags_scm = loader.get_tags(grammar_name)?;
                let ts_lang = loader.get(grammar_name)?;
                let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;

                let capture_names = tags_query.capture_names();
                let root_node = tree.root_node();
                let mut qcursor = tree_sitter::QueryCursor::new();
                let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());

                let mut count = 0usize;
                let mut seen: HashSet<(usize, usize)> = HashSet::new();
                while let Some(m) = matches.next() {
                    for capture in m.captures {
                        let cn = capture_names[capture.index as usize];
                        if matches!(cn, "definition.class" | "definition.interface") {
                            let key = (capture.node.start_byte(), capture.node.end_byte());
                            if seen.insert(key) {
                                count += 1;
                            }
                        }
                    }
                }
                Some((f.path.replace('\\', "/"), count as f64))
            })
            .collect();
        Ok(results)
    }
}

impl Metric for CommentLineCountMetric {
    fn name(&self) -> &'static str {
        "comment-line-count"
    }

    fn measure_all(&self, root: &Path) -> anyhow::Result<Vec<(String, f64)>> {
        let all_files = normalize_path_resolve::all_files(root, None);
        let results: Vec<(String, f64)> = all_files
            .par_iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| {
                let abs_path = root.join(&f.path);
                let support = support_for_path(&abs_path)?;
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let grammar_name = support.grammar_name();
                let tree = normalize_facts::parse_with_grammar(grammar_name, &content)?;
                let count = count_comment_lines(&tree, &content);
                Some((f.path.replace('\\', "/"), count as f64))
            })
            .collect();
        Ok(results)
    }
}

/// Count lines containing comment nodes by traversing the tree.
fn count_comment_lines(tree: &tree_sitter::Tree, content: &str) -> usize {
    let mut comment_lines: HashSet<usize> = HashSet::new();
    traverse_comments(tree.root_node(), content, &mut comment_lines);
    comment_lines.len()
}

fn traverse_comments(node: tree_sitter::Node, content: &str, comment_lines: &mut HashSet<usize>) {
    let kind = node.kind();
    if kind.contains("comment")
        || kind == "doc_comment"
        || kind == "block_comment"
        || kind == "line_comment"
    {
        let start_row = node.start_position().row;
        let end_row = node.end_position().row;
        let _ = content; // content not needed for line counting
        for row in start_row..=end_row {
            comment_lines.insert(row);
        }
        return; // Don't recurse into comments
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse_comments(cursor.node(), content, comment_lines);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
