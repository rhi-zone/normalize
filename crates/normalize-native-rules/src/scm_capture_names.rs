//! Native rule: validate capture names in tree-sitter `.calls.scm` query files.
//!
//! The facts system indexes call data using exactly two capture names: `@call` and
//! `@call.qualifier`.  Any other capture name in a `.calls.scm` file is silently
//! ignored at index time — it neither errors nor contributes data.  This was the
//! root cause of a bug where `call_complexity.rs` looked for a `"reference.call"`
//! capture that the facts system never produced.
//!
//! This check scans every `*.calls.scm` file under the project root and flags any
//! capture name that is not in the expected set.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single unexpected capture name found in a `.calls.scm` file.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct UnexpectedCapture {
    file: String,
    line: usize,
    capture: String,
}

/// Report produced by the `scm-capture-names` native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ScmCaptureNamesReport {
    unexpected: Vec<UnexpectedCapture>,
    files_checked: usize,
}

impl OutputFormatter for ScmCaptureNamesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("SCM Capture Names Check".to_string());
        lines.push(String::new());
        lines.push(format!(".calls.scm files checked: {}", self.files_checked));
        lines.push(String::new());

        if self.unexpected.is_empty() {
            lines.push("All .calls.scm capture names are valid.".to_string());
        } else {
            lines.push(format!(
                "Unexpected capture names ({}):",
                self.unexpected.len()
            ));
            for u in &self.unexpected {
                lines.push(format!(
                    "  {}:{}: `{}` (expected: @call, @call.qualifier)",
                    u.file, u.line, u.capture
                ));
            }
        }

        lines.join("\n")
    }
}

/// Capture names that the facts system recognises in `.calls.scm` files.
const ALLOWED_CALLS_CAPTURES: &[&str] = &["call", "call.qualifier"];

/// Find all `.calls.scm` files under `root` (respecting `.gitignore`) and
/// check that every capture they use is in the allowed set for that query type.
///
/// Only `.calls.scm` files are validated strictly because that is the proven
/// source of silent indexing bugs.  Other query types (`tags`, `imports`,
/// `complexity`, `types`) are not yet validated.
pub fn build_scm_capture_names_report(root: &Path) -> ScmCaptureNamesReport {
    let mut unexpected = Vec::new();
    let mut files_checked = 0usize;

    for entry in crate::walk::gitignore_walk(root) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Only validate .calls.scm files for now.
        if !name.ends_with(".calls.scm") {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("normalize-native-rules: could not read {:?}: {}", path, e);
                continue;
            }
        };

        files_checked += 1;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        for (line_idx, line) in content.lines().enumerate() {
            // Skip comment lines (tree-sitter .scm comments start with ';').
            let trimmed = line.trim_start();
            if trimmed.starts_with(';') {
                continue;
            }

            // Extract all @capture_name tokens from the line.
            let mut rest = line;
            while let Some(at_pos) = rest.find('@') {
                rest = &rest[at_pos + 1..];
                // A capture name consists of alphanumerics, underscores, hyphens,
                // and dots.  Stop at the first character that doesn't fit.
                let end = rest
                    .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.')
                    .unwrap_or(rest.len());
                if end == 0 {
                    continue;
                }
                let capture = &rest[..end];
                rest = &rest[end..];

                if !ALLOWED_CALLS_CAPTURES.contains(&capture) {
                    unexpected.push(UnexpectedCapture {
                        file: rel_path.clone(),
                        line: line_idx + 1,
                        capture: format!("@{}", capture),
                    });
                }
            }
        }
    }

    ScmCaptureNamesReport {
        unexpected,
        files_checked,
    }
}

impl From<ScmCaptureNamesReport> for DiagnosticsReport {
    fn from(report: ScmCaptureNamesReport) -> Self {
        let issues = report
            .unexpected
            .into_iter()
            .map(|u| Issue {
                file: u.file,
                line: Some(u.line),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "scm-capture-names".into(),
                message: format!(
                    "unexpected capture `{}` in .calls.scm file (allowed: @call, @call.qualifier)",
                    u.capture
                ),
                severity: Severity::Warning,
                source: "scm-capture-names".into(),
                related: vec![],
                suggestion: Some(
                    "rename the capture to @call or @call.qualifier, or remove it if unused".into(),
                ),
            })
            .collect::<Vec<_>>();

        let files_checked = report.files_checked;
        DiagnosticsReport {
            issues,
            files_checked,
            sources_run: vec!["scm-capture-names".into()],
            tool_errors: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_file(dir: &std::path::Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn valid_captures_pass() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            tmp.path(),
            "rust.calls.scm",
            r#"
(call_expression
  function: (identifier) @call
  receiver: (_) @call.qualifier)
"#,
        );
        let report = build_scm_capture_names_report(tmp.path());
        assert_eq!(report.files_checked, 1);
        assert!(
            report.unexpected.is_empty(),
            "expected no issues, got: {:?}",
            report.unexpected
        );
    }

    #[test]
    fn invalid_capture_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            tmp.path(),
            "python.calls.scm",
            r#"
(call
  function: (identifier) @reference.call)
"#,
        );
        let report = build_scm_capture_names_report(tmp.path());
        assert_eq!(report.files_checked, 1);
        assert_eq!(report.unexpected.len(), 1);
        assert_eq!(report.unexpected[0].capture, "@reference.call");
    }

    #[test]
    fn comment_lines_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            tmp.path(),
            "go.calls.scm",
            r#"
; This is a comment mentioning @reference.call — should be ignored
(call_expression
  function: (identifier) @call)
"#,
        );
        let report = build_scm_capture_names_report(tmp.path());
        assert_eq!(report.files_checked, 1);
        assert!(
            report.unexpected.is_empty(),
            "comment captures should not be flagged"
        );
    }

    #[test]
    fn non_calls_scm_files_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            tmp.path(),
            "rust.tags.scm",
            r#"
(function_item
  name: (identifier) @definition.function)
"#,
        );
        let report = build_scm_capture_names_report(tmp.path());
        assert_eq!(report.files_checked, 0, "tags.scm should not be checked");
        assert!(report.unexpected.is_empty());
    }

    #[test]
    fn diagnostics_report_conversion() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            tmp.path(),
            "ts.calls.scm",
            "(call_expression function: (identifier) @bad.capture)",
        );
        let report = build_scm_capture_names_report(tmp.path());
        let diag: DiagnosticsReport = report.into();
        assert_eq!(diag.issues.len(), 1);
        assert_eq!(diag.issues[0].rule_id, "scm-capture-names");
    }
}
