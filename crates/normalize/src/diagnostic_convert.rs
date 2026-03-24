//! Conversions from domain-specific diagnostic types to the unified `Issue` type.
//!
//! These live in the main crate because `normalize-output` (which defines `Issue`)
//! cannot depend on `normalize-syntax-rules` or `normalize-facts-rules-api`.

use normalize_output::diagnostics::{Issue, RelatedLocation, Severity};

/// Convert a syntax-rules `Finding` into a unified `Issue`.
pub fn finding_to_issue(f: &normalize_syntax_rules::Finding, root: &std::path::Path) -> Issue {
    let rel_path = f.file.strip_prefix(root).unwrap_or(&f.file);
    Issue {
        file: rel_path.to_string_lossy().to_string(),
        line: Some(f.start_line),
        column: Some(f.start_col),
        end_line: Some(f.end_line),
        end_column: Some(f.end_col),
        rule_id: f.rule_id.clone(),
        message: f.message.clone(),
        severity: syntax_severity(f.severity),
        source: "syntax-rules".into(),
        related: Vec::new(),
        suggestion: f.fix.clone(),
    }
}

/// Convert syntax-rules `Severity` to output `Severity`.
fn syntax_severity(s: normalize_syntax_rules::Severity) -> Severity {
    match s {
        normalize_syntax_rules::Severity::Error => Severity::Error,
        normalize_syntax_rules::Severity::Warning => Severity::Warning,
        normalize_syntax_rules::Severity::Info => Severity::Info,
        normalize_syntax_rules::Severity::Hint => Severity::Hint,
    }
}

/// Convert a facts-rules-api `Diagnostic` into a unified `Issue`.
pub fn abi_diagnostic_to_issue(d: &normalize_facts_rules_api::Diagnostic) -> Issue {
    use abi_stable::std_types::ROption;

    let (file, line, column) = match &d.location {
        ROption::RSome(loc) => (
            loc.file.to_string(),
            Some(loc.line as usize),
            match &loc.column {
                ROption::RSome(c) => Some(*c as usize),
                ROption::RNone => None,
            },
        ),
        ROption::RNone => (String::new(), None, None),
    };

    let related = d
        .related
        .iter()
        .map(|loc| RelatedLocation {
            file: loc.file.to_string(),
            line: Some(loc.line as usize),
            message: None,
        })
        .collect();

    let suggestion = match &d.suggestion {
        ROption::RSome(s) => Some(s.to_string()),
        ROption::RNone => None,
    };

    Issue {
        file,
        line,
        column,
        end_line: None,
        end_column: None,
        rule_id: d.rule_id.to_string(),
        message: d.message.to_string(),
        severity: abi_level(d.level),
        source: "fact-rules".into(),
        related,
        suggestion,
    }
}

/// Convert facts-rules-api `DiagnosticLevel` to output `Severity`.
fn abi_level(level: normalize_facts_rules_api::DiagnosticLevel) -> Severity {
    match level {
        normalize_facts_rules_api::DiagnosticLevel::Hint => Severity::Hint,
        normalize_facts_rules_api::DiagnosticLevel::Warning => Severity::Warning,
        normalize_facts_rules_api::DiagnosticLevel::Error => Severity::Error,
    }
}
