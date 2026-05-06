//! `long-function` native rule — flags functions exceeding a line count threshold.
//!
//! Uses tree-sitter tags queries to identify function boundaries and measures
//! line span (end_line - start_line + 1).
//!
//! # Configuration
//!
//! The threshold is configurable via `.normalize/config.toml`:
//!
//! ```toml
//! [rules.rule."long-function"]
//! threshold = 50   # default: 100
//! ```

use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::support_for_path;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use std::path::Path;
use streaming_iterator::StreamingIterator;

use crate::cache::{FileRule, run_file_rule};
use normalize_rules_config::WalkConfig;

/// Serializable per-file finding for the long-function rule.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct LongFunctionFinding {
    rel_path: String,
    name: String,
    start_line: usize,
    line_count: usize,
}

/// Rule that flags functions exceeding a line count threshold.
pub struct LongFunctionRule {
    pub threshold: usize,
}

impl FileRule for LongFunctionRule {
    type Finding = LongFunctionFinding;

    fn engine_name(&self) -> &str {
        "long-function"
    }

    fn config_hash(&self) -> String {
        self.threshold.to_string()
    }

    fn check_file(&self, path: &Path, root: &Path) -> Vec<Self::Finding> {
        let support = match support_for_path(path) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let grammar_name = support.grammar_name();
        let tree = match parse_with_grammar(grammar_name, &content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let loader = grammar_loader();
        let tags_scm = match loader.get_tags(grammar_name) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let ts_lang = match loader.get(grammar_name) {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };
        let tags_query = match tree_sitter::Query::new(&ts_lang, &tags_scm) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };

        let capture_names = tags_query.capture_names();
        let root_node = tree.root_node();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(&tags_query, root_node, content.as_bytes());

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let mut results = Vec::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let cn = capture_names[capture.index as usize];
                if !matches!(cn, "definition.function" | "definition.method") {
                    continue;
                }

                let node = capture.node;
                let name = match support.node_name(&node, &content) {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let line_count = end_line.saturating_sub(start_line) + 1;

                if line_count >= self.threshold {
                    results.push(LongFunctionFinding {
                        rel_path: rel_path.clone(),
                        name,
                        start_line,
                        line_count,
                    });
                }
            }
        }

        results
    }

    fn to_diagnostics(
        &self,
        findings: Vec<(std::path::PathBuf, Vec<Self::Finding>)>,
        _root: &Path,
        files_checked: usize,
    ) -> DiagnosticsReport {
        let threshold = self.threshold;

        let mut issues: Vec<Issue> = findings
            .into_iter()
            .flat_map(|(_path, file_findings)| file_findings)
            .map(|f| Issue {
                file: f.rel_path,
                line: Some(f.start_line),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "long-function".into(),
                message: format!(
                    "function `{}` is {} lines (threshold: {threshold})",
                    f.name, f.line_count
                ),
                severity: Severity::Warning,
                source: "long-function".into(),
                related: vec![],
                suggestion: Some(
                    "consider breaking this function into smaller, focused functions".into(),
                ),
            })
            .collect();

        // Sort by line count descending.
        issues.sort_by(|a, b| {
            let extract = |msg: &str| -> usize {
                msg.split(" is ")
                    .nth(1)
                    .and_then(|s| s.split(' ').next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0)
            };
            extract(&b.message).cmp(&extract(&a.message))
        });

        DiagnosticsReport {
            issues,
            files_checked,
            sources_run: vec!["long-function".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}

/// Build a `DiagnosticsReport` for the `long-function` rule.
///
/// Walks all source files under `root`, parses each with tree-sitter, and emits
/// an issue for every function whose line span meets or exceeds the threshold.
pub fn build_long_function_report(
    root: &Path,
    threshold: usize,
    explicit_files: Option<&[std::path::PathBuf]>,
    walk_config: &WalkConfig,
) -> DiagnosticsReport {
    let rule = LongFunctionRule { threshold };
    run_file_rule(&rule, root, explicit_files, walk_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// Write a Python file with a single function spanning `body_lines` lines.
    fn make_python_function(
        dir: &std::path::Path,
        name: &str,
        body_lines: usize,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "def long_function():").unwrap();
        for i in 0..body_lines {
            writeln!(f, "    x = {i}").unwrap();
        }
        path
    }

    #[test]
    fn test_default_threshold_not_triggered() {
        let dir = tempfile::tempdir().unwrap();
        // 99-line body → 100 total lines but function span is 100 (threshold is >=)
        let path = make_python_function(dir.path(), "short.py", 98);
        let rule = LongFunctionRule { threshold: 100 };
        let findings = rule.check_file(&path, dir.path());
        assert!(
            findings.is_empty(),
            "99-line function should not trigger default threshold of 100"
        );
    }

    #[test]
    fn test_default_threshold_triggered() {
        let dir = tempfile::tempdir().unwrap();
        // 100-line body → function span >= 100
        let path = make_python_function(dir.path(), "long.py", 100);
        let rule = LongFunctionRule { threshold: 100 };
        let findings = rule.check_file(&path, dir.path());
        assert!(
            !findings.is_empty(),
            "100-line function should trigger threshold of 100"
        );
    }

    #[test]
    fn test_custom_threshold_lower() {
        let dir = tempfile::tempdir().unwrap();
        // 30-line body — below default (100) but above custom threshold of 20
        let path = make_python_function(dir.path(), "medium.py", 30);
        let rule = LongFunctionRule { threshold: 20 };
        let findings = rule.check_file(&path, dir.path());
        assert!(
            !findings.is_empty(),
            "30-line function should trigger custom threshold of 20"
        );
    }

    #[test]
    fn test_custom_threshold_higher() {
        let dir = tempfile::tempdir().unwrap();
        // 100-line body — at default (100) but below custom threshold of 200
        let path = make_python_function(dir.path(), "medium.py", 100);
        let rule = LongFunctionRule { threshold: 200 };
        let findings = rule.check_file(&path, dir.path());
        assert!(
            findings.is_empty(),
            "100-line function should not trigger custom threshold of 200"
        );
    }
}
