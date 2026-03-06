//! Unified diagnostic types for all issue-reporting commands.
//!
//! Any command that finds "problems in files" — broken references, stale docs,
//! missing examples, security findings, lint violations, rule matches — should
//! converge on these types.

use crate::OutputFormatter;
use serde::{Deserialize, Serialize};

/// Severity level for a diagnostic issue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Hint => write!(f, "hint"),
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A secondary location related to an issue (e.g., the other file in a circular dep).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RelatedLocation {
    pub file: String,
    pub line: Option<usize>,
    pub message: Option<String>,
}

/// A single diagnostic issue found during a check.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub file: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    /// Which engine/check produced this issue.
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Issue {
    /// Format as `file:line:col: severity [rule_id] message`.
    pub fn format_location(&self) -> String {
        let mut loc = self.file.clone();
        if let Some(line) = self.line {
            loc.push_str(&format!(":{line}"));
            if let Some(col) = self.column {
                loc.push_str(&format!(":{col}"));
            }
        }
        loc
    }
}

/// Report containing diagnostic issues from one or more checks.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiagnosticsReport {
    pub issues: Vec<Issue>,
    pub files_checked: usize,
    /// Which checks/engines produced issues in this report.
    pub sources_run: Vec<String>,
}

impl DiagnosticsReport {
    /// Create an empty report.
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            files_checked: 0,
            sources_run: Vec::new(),
        }
    }

    /// Merge another report into this one.
    pub fn merge(&mut self, other: DiagnosticsReport) {
        self.files_checked = self.files_checked.max(other.files_checked);
        self.issues.extend(other.issues);
        for source in other.sources_run {
            if !self.sources_run.contains(&source) {
                self.sources_run.push(source);
            }
        }
    }

    /// Sort issues by file, then line, then severity (most severe first).
    pub fn sort(&mut self) {
        self.issues.sort_by(|a, b| {
            a.file
                .cmp(&b.file)
                .then(a.line.cmp(&b.line))
                .then(b.severity.cmp(&a.severity))
        });
    }

    /// Format as SARIF 2.1.0 JSON.
    pub fn format_sarif(&self) -> String {
        // Collect unique rule IDs to build the tool.driver.rules array
        let mut rule_ids: Vec<String> = Vec::new();
        for issue in &self.issues {
            if !rule_ids.contains(&issue.rule_id) {
                rule_ids.push(issue.rule_id.clone());
            }
        }

        let sarif_rules: Vec<serde_json::Value> = rule_ids
            .iter()
            .map(|id| {
                // Find the first issue with this rule_id to derive default severity
                let first = self.issues.iter().find(|i| &i.rule_id == id);
                let level = first.map_or("warning", |i| severity_to_sarif_level(i.severity));
                serde_json::json!({
                    "id": id,
                    "defaultConfiguration": { "level": level }
                })
            })
            .collect();

        let results: Vec<serde_json::Value> = self
            .issues
            .iter()
            .map(|issue| {
                let mut region = serde_json::Map::new();
                if let Some(line) = issue.line {
                    region.insert("startLine".into(), serde_json::json!(line));
                }
                if let Some(col) = issue.column {
                    region.insert("startColumn".into(), serde_json::json!(col));
                }
                if let Some(end_line) = issue.end_line {
                    region.insert("endLine".into(), serde_json::json!(end_line));
                }
                if let Some(end_col) = issue.end_column {
                    region.insert("endColumn".into(), serde_json::json!(end_col));
                }

                serde_json::json!({
                    "ruleId": issue.rule_id,
                    "level": severity_to_sarif_level(issue.severity),
                    "message": { "text": issue.message },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": issue.file },
                            "region": region
                        }
                    }]
                })
            })
            .collect();

        let sarif = serde_json::json!({
            "version": "2.1.0",
            "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "normalize",
                        "informationUri": "https://github.com/rhi-zone/normalize",
                        "rules": sarif_rules
                    }
                },
                "results": results
            }]
        });

        serde_json::to_string_pretty(&sarif).unwrap()
    }

    /// Count issues by severity.
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == severity)
            .count()
    }
}

impl Default for DiagnosticsReport {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticsReport {
    /// Format as text with an optional limit on the number of issues shown.
    /// Only errors and warnings are shown in detail; info/hints are summarized at the end.
    pub fn format_text_limited(&self, limit: Option<usize>) -> String {
        let mut out = String::new();
        if self.issues.is_empty() {
            out.push_str(&format!(
                "No issues found ({} files checked, sources: {}).\n",
                self.files_checked,
                self.sources_run.join(", ")
            ));
            return out;
        }

        let errors = self.count_by_severity(Severity::Error);
        let warnings = self.count_by_severity(Severity::Warning);
        let infos = self.count_by_severity(Severity::Info);
        let hints = self.count_by_severity(Severity::Hint);
        let actionable = errors + warnings;

        // Header counts all issues for complete picture
        let files_str = if self.files_checked > 0 {
            format!("{} files", self.files_checked)
        } else {
            format!("sources: {}", self.sources_run.join(", "))
        };
        out.push_str(&format!("{} issues ({})\n", self.issues.len(), files_str));

        let mut parts = Vec::new();
        if errors > 0 {
            parts.push(format!(
                "{errors} error{}",
                if errors == 1 { "" } else { "s" }
            ));
        }
        if warnings > 0 {
            parts.push(format!(
                "{warnings} warning{}",
                if warnings == 1 { "" } else { "s" }
            ));
        }
        if infos > 0 {
            parts.push(format!("{infos} info"));
        }
        if hints > 0 {
            parts.push(format!("{hints} hint{}", if hints == 1 { "" } else { "s" }));
        }
        if !parts.is_empty() {
            out.push_str(&format!("  {}\n", parts.join(", ")));
        }
        out.push('\n');

        // Only show errors and warnings in detail; info/hints are noisy and informational only
        let actionable_issues: Vec<&Issue> = self
            .issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Error | Severity::Warning))
            .collect();

        let shown = if let Some(lim) = limit {
            actionable_issues.len().min(lim)
        } else {
            actionable_issues.len()
        };

        for issue in actionable_issues.iter().take(shown) {
            out.push_str(&format!(
                "{}: {} [{}] {}\n",
                issue.format_location(),
                issue.severity,
                issue.rule_id,
                issue.message,
            ));
            for rel in &issue.related {
                let rloc = if let Some(line) = rel.line {
                    format!("{}:{line}", rel.file)
                } else {
                    rel.file.clone()
                };
                if let Some(msg) = &rel.message {
                    out.push_str(&format!("  --> {rloc}: {msg}\n"));
                } else {
                    out.push_str(&format!("  --> {rloc}\n"));
                }
            }
            if let Some(suggestion) = &issue.suggestion {
                out.push_str(&format!("  suggestion: {suggestion}\n"));
            }
        }

        if shown < actionable {
            out.push_str(&format!(
                "  ... {} more not shown (use --limit or --pretty to see all)\n",
                actionable - shown
            ));
        }
        if infos + hints > 0 {
            out.push_str(&format!(
                "  {} info/hint suggestion{} (use --pretty to show)\n",
                infos + hints,
                if infos + hints == 1 { "" } else { "s" }
            ));
        }

        out
    }
}

impl OutputFormatter for DiagnosticsReport {
    fn format_text(&self) -> String {
        self.format_text_limited(None)
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;

        if self.issues.is_empty() {
            return format!(
                "{} No issues found ({} files checked)\n",
                Color::Green.paint("✓"),
                self.files_checked
            );
        }

        let mut out = String::new();
        let errors = self.count_by_severity(Severity::Error);
        let warnings = self.count_by_severity(Severity::Warning);

        let header_color = if errors > 0 {
            Color::Red
        } else {
            Color::Yellow
        };
        out.push_str(&format!(
            "{}\n",
            header_color.bold().paint(format!(
                "{} issues ({} files checked)",
                self.issues.len(),
                self.files_checked
            ))
        ));
        let mut parts = Vec::new();
        if errors > 0 {
            parts.push(
                Color::Red
                    .paint(format!(
                        "{errors} error{}",
                        if errors == 1 { "" } else { "s" }
                    ))
                    .to_string(),
            );
        }
        if warnings > 0 {
            parts.push(
                Color::Yellow
                    .paint(format!(
                        "{warnings} warning{}",
                        if warnings == 1 { "" } else { "s" }
                    ))
                    .to_string(),
            );
        }
        let infos = self.count_by_severity(Severity::Info);
        let hints = self.count_by_severity(Severity::Hint);
        if infos > 0 {
            parts.push(format!("{infos} info"));
        }
        if hints > 0 {
            parts.push(format!("{hints} hint{}", if hints == 1 { "" } else { "s" }));
        }
        if !parts.is_empty() {
            out.push_str(&format!("  {}\n", parts.join(", ")));
        }
        out.push('\n');

        for issue in &self.issues {
            let sev_color = match issue.severity {
                Severity::Error => Color::Red,
                Severity::Warning => Color::Yellow,
                Severity::Info => Color::Cyan,
                Severity::Hint => Color::DarkGray,
            };
            out.push_str(&format!(
                "{}: {} {} {}\n",
                Color::White.bold().paint(issue.format_location()),
                sev_color.bold().paint(issue.severity.to_string()),
                Color::DarkGray.paint(format!("[{}]", issue.rule_id)),
                issue.message,
            ));
            for rel in &issue.related {
                let rloc = if let Some(line) = rel.line {
                    format!("{}:{line}", rel.file)
                } else {
                    rel.file.clone()
                };
                if let Some(msg) = &rel.message {
                    out.push_str(&format!(
                        "  {} {}: {msg}\n",
                        Color::DarkGray.paint("-->"),
                        rloc
                    ));
                } else {
                    out.push_str(&format!("  {} {}\n", Color::DarkGray.paint("-->"), rloc));
                }
            }
            if let Some(suggestion) = &issue.suggestion {
                out.push_str(&format!(
                    "  {} {suggestion}\n",
                    Color::Green.paint("suggestion:")
                ));
            }
        }

        out
    }
}

/// Convert diagnostic `Severity` to SARIF level string.
fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
        Severity::Hint => "note",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_report() {
        let report = DiagnosticsReport {
            issues: vec![],
            files_checked: 10,
            sources_run: vec!["check-refs".into()],
        };
        let text = report.format_text();
        assert!(text.contains("No issues found"));
        assert!(text.contains("10 files checked"));
    }

    #[test]
    fn test_issue_format_location() {
        let issue = Issue {
            file: "src/main.rs".into(),
            line: Some(42),
            column: Some(5),
            end_line: None,
            end_column: None,
            rule_id: "broken-ref".into(),
            message: "Unknown symbol `Foo`".into(),
            severity: Severity::Warning,
            source: "check-refs".into(),
            related: vec![],
            suggestion: None,
        };
        assert_eq!(issue.format_location(), "src/main.rs:42:5");
    }

    #[test]
    fn test_issue_format_location_no_col() {
        let issue = Issue {
            file: "docs/README.md".into(),
            line: Some(10),
            column: None,
            end_line: None,
            end_column: None,
            rule_id: "stale-doc".into(),
            message: "Doc is stale".into(),
            severity: Severity::Info,
            source: "stale-docs".into(),
            related: vec![],
            suggestion: None,
        };
        assert_eq!(issue.format_location(), "docs/README.md:10");
    }

    #[test]
    fn test_report_merge() {
        let mut a = DiagnosticsReport {
            issues: vec![Issue {
                file: "a.rs".into(),
                line: Some(1),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "r1".into(),
                message: "msg1".into(),
                severity: Severity::Warning,
                source: "check-refs".into(),
                related: vec![],
                suggestion: None,
            }],
            files_checked: 5,
            sources_run: vec!["check-refs".into()],
        };
        let b = DiagnosticsReport {
            issues: vec![Issue {
                file: "b.rs".into(),
                line: Some(2),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "r2".into(),
                message: "msg2".into(),
                severity: Severity::Error,
                source: "stale-docs".into(),
                related: vec![],
                suggestion: None,
            }],
            files_checked: 8,
            sources_run: vec!["stale-docs".into()],
        };
        a.merge(b);
        assert_eq!(a.issues.len(), 2);
        assert_eq!(a.files_checked, 8);
        assert_eq!(a.sources_run, vec!["check-refs", "stale-docs"]);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert!(Severity::Info > Severity::Hint);
    }

    #[test]
    fn test_report_sort() {
        let mut report = DiagnosticsReport {
            issues: vec![
                Issue {
                    file: "b.rs".into(),
                    line: Some(1),
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "r1".into(),
                    message: "m".into(),
                    severity: Severity::Warning,
                    source: "s".into(),
                    related: vec![],
                    suggestion: None,
                },
                Issue {
                    file: "a.rs".into(),
                    line: Some(1),
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "r2".into(),
                    message: "m".into(),
                    severity: Severity::Error,
                    source: "s".into(),
                    related: vec![],
                    suggestion: None,
                },
            ],
            files_checked: 2,
            sources_run: vec!["s".into()],
        };
        report.sort();
        assert_eq!(report.issues[0].file, "a.rs");
        assert_eq!(report.issues[1].file, "b.rs");
    }
}
