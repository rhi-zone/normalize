//! Validate example references in documentation

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::Serialize;
use std::path::Path;

static MARKER_START_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// A missing example reference
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct MissingExample {
    doc_file: String,
    line: usize,
    reference: String, // path#name
}

/// Report produced by the missing-example native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CheckExamplesReport {
    defined_examples: usize,
    references_found: usize,
    missing: Vec<MissingExample>,
}

impl OutputFormatter for CheckExamplesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Example Reference Check".to_string());
        lines.push(String::new());
        lines.push(format!("Defined examples: {}", self.defined_examples));
        lines.push(format!("References found: {}", self.references_found));
        lines.push(String::new());

        if self.missing.is_empty() {
            lines.push("All example references are valid.".to_string());
        } else {
            lines.push(format!("Missing examples ({}):", self.missing.len()));
            lines.push(String::new());
            for m in &self.missing {
                lines.push(format!(
                    "  {}:{}: {{{{{}}}}}",
                    m.doc_file, m.line, m.reference
                ));
            }
        }

        lines.join("\n")
    }
}

/// Build a CheckExamplesReport without printing (for service layer).
pub fn build_check_examples_report(
    root: &Path,
    walk_config: &normalize_rules_config::WalkConfig,
) -> CheckExamplesReport {
    use std::collections::HashSet;

    // normalize-syntax-allow: rust/unwrap-in-impl - compile-time-known-valid regex
    let marker_start_re =
        MARKER_START_RE.get_or_init(|| regex::Regex::new(r"//\s*\[example:\s*([^\]]+)\]").unwrap());
    // normalize-syntax-allow: rust/unwrap-in-impl - compile-time-known-valid regex
    let ref_re = REF_RE.get_or_init(|| regex::Regex::new(r"\{\{example:\s*([^}]+)\}\}").unwrap());

    let mut defined_examples: HashSet<String> = HashSet::new();

    for entry in crate::walk::gitignore_walk(root, walk_config)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
    {
        let path = entry.path();

        if normalize_languages::support_for_path(path).is_none() {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        for cap in marker_start_re.captures_iter(&content) {
            let name = cap[1].trim();
            let key = format!("{}#{}", rel_path, name);
            defined_examples.insert(key);
        }
    }

    let mut missing: Vec<MissingExample> = Vec::new();
    let mut refs_found = 0;

    for entry in crate::walk::gitignore_walk(root, walk_config)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let mut in_code_block = false;
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }

            for cap in ref_re.captures_iter(line) {
                // normalize-syntax-allow: rust/unwrap-in-impl - cap.get(0) is always Some (the full match)
                let match_start = cap.get(0).unwrap().start();
                let match_end = cap.get(0).unwrap().end();
                // SAFETY: regex byte offsets are always at UTF-8 char boundaries for valid UTF-8 input
                let before = &line[..match_start];
                let after = &line[match_end..];

                if before.chars().filter(|&c| c == '`').count() % 2 == 1 && after.contains('`') {
                    continue;
                }

                refs_found += 1;
                let reference = cap[1].trim();

                if !defined_examples.contains(reference) {
                    missing.push(MissingExample {
                        doc_file: rel_path.clone(),
                        line: line_num + 1,
                        reference: reference.to_string(),
                    });
                }
            }
        }
    }

    CheckExamplesReport {
        defined_examples: defined_examples.len(),
        references_found: refs_found,
        missing,
    }
}

impl From<CheckExamplesReport> for DiagnosticsReport {
    fn from(report: CheckExamplesReport) -> Self {
        DiagnosticsReport {
            issues: report
                .missing
                .into_iter()
                .map(|m| Issue {
                    file: m.doc_file,
                    line: Some(m.line),
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "missing-example".into(),
                    message: format!("example `{}` not found in source", m.reference),
                    severity: Severity::Warning,
                    source: "check-examples".into(),
                    related: vec![],
                    suggestion: None,
                })
                .collect(),
            files_checked: 0, // not tracked separately in CheckExamplesReport
            sources_run: vec!["check-examples".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}
