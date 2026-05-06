//! `boundary-violations` native rule — flags cross-boundary imports.
//!
//! Users declare directory-level import boundaries in config. The rule checks
//! all resolved imports in the structural index and reports any that violate a
//! declared boundary.
//!
//! # Configuration
//!
//! ```toml
//! [rules.rule."boundary-violations"]
//! enabled = true
//! boundaries = [
//!   "services/ cannot import cli/",
//!   "crates/normalize-facts/ cannot import crates/normalize/",
//! ]
//! ```
//!
//! Each boundary string has the form `"<from_glob> cannot import <to_glob>"`.
//! Both globs are matched against root-relative file paths using [`glob::Pattern`].
//! Trailing `/` is allowed — it is treated as a prefix match (the pattern
//! `services/` matches any path that starts with `services/`).

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity, ToolFailure};
use std::path::Path;

/// A single configured boundary constraint.
#[derive(Debug, Clone)]
pub struct Boundary {
    pub from_glob: String,
    pub to_glob: String,
    /// The original string the user wrote (for error messages).
    pub raw: String,
}

/// Config for the `boundary-violations` rule, deserialized from
/// `[rules.rule."boundary-violations"]` in `.normalize/config.toml`.
#[derive(serde::Deserialize, Default, Debug)]
pub struct BoundaryViolationsConfig {
    /// Boundary strings in the form `"<from> cannot import <to>"`.
    #[serde(default)]
    pub boundaries: Vec<String>,
}

/// One boundary-violation finding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoundaryViolationFinding {
    /// Root-relative path of the importing file.
    pub importer: String,
    /// Line of the import statement.
    pub line: u32,
    /// Root-relative path of the imported file.
    pub imported: String,
    /// The violated boundary string (for the message).
    pub boundary: String,
}

/// Parse a boundary string of the form `"A cannot import B"`.
/// Returns `None` if the string doesn't match the expected format.
pub fn parse_boundary(s: &str) -> Option<Boundary> {
    let sep = " cannot import ";
    let pos = s.find(sep)?;
    let from_glob = s[..pos].trim().to_string();
    let to_glob = s[pos + sep.len()..].trim().to_string();
    if from_glob.is_empty() || to_glob.is_empty() {
        return None;
    }
    Some(Boundary {
        from_glob,
        to_glob,
        raw: s.to_string(),
    })
}

/// Expand a glob-like pattern that may end with `/` into a `glob::Pattern`
/// suitable for matching root-relative file paths.
///
/// A trailing `/` means "any file under this directory prefix", so
/// `services/` → `services/**`. Patterns that already contain `*` are passed
/// through unchanged.
fn compile_glob(raw: &str) -> Option<glob::Pattern> {
    let expanded = if raw.ends_with('/') && !raw.contains('*') {
        format!("{}**", raw)
    } else {
        raw.to_string()
    };
    glob::Pattern::new(&expanded).ok()
}

/// Check whether a root-relative path matches a boundary side's glob.
fn matches_glob(pattern: &glob::Pattern, path: &str) -> bool {
    pattern.matches(path)
        || pattern.matches_with(
            path,
            glob::MatchOptions {
                case_sensitive: true,
                require_literal_separator: false,
                require_literal_leading_dot: false,
            },
        )
}

/// Build a `DiagnosticsReport` for the `boundary-violations` rule.
///
/// Requires the structural index (run `normalize structure rebuild` first).
/// Returns an empty report (with a hint in `tool_errors`) if the index is
/// absent or the boundaries list is empty.
pub async fn build_boundary_violations_report(
    root: &Path,
    boundaries: &[Boundary],
) -> DiagnosticsReport {
    let mut report = DiagnosticsReport::new();

    if boundaries.is_empty() {
        return report;
    }

    // Compile glob patterns once.
    let compiled: Vec<(glob::Pattern, glob::Pattern, &Boundary)> = boundaries
        .iter()
        .filter_map(|b| {
            let from_pat = compile_glob(&b.from_glob)?;
            let to_pat = compile_glob(&b.to_glob)?;
            Some((from_pat, to_pat, b))
        })
        .collect();

    if compiled.is_empty() {
        return report;
    }

    // Open the structural index.
    let db_path = crate::check_refs::normalize_dir_for_root(root).join("index.sqlite");
    let idx = match normalize_facts::FileIndex::open(&db_path, root).await {
        Ok(idx) => idx,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "boundary-violations".into(),
                message: format!(
                    "failed to open index at {}: {}. Run `normalize structure rebuild` first.",
                    db_path.display(),
                    e
                ),
            });
            return report;
        }
    };

    // Load all resolved import edges with line numbers.
    let edges = match idx.all_resolved_imports_with_lines().await {
        Ok(edges) => edges,
        Err(e) => {
            report.tool_errors.push(ToolFailure {
                tool: "boundary-violations".into(),
                message: format!("failed to query imports table: {e}"),
            });
            return report;
        }
    };

    // For each edge, check against every boundary.
    for (importer, line, imported) in &edges {
        for (from_pat, to_pat, boundary) in &compiled {
            if matches_glob(from_pat, importer) && matches_glob(to_pat, imported) {
                report.issues.push(Issue {
                    file: importer.clone(),
                    line: Some(*line as usize),
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "boundary-violations".into(),
                    message: format!(
                        "imports `{}` — violates boundary: {}",
                        imported, boundary.raw
                    ),
                    severity: Severity::Warning,
                    source: "boundary-violations".into(),
                    related: vec![],
                    suggestion: Some(
                        "move shared code to a layer both sides may depend on, or revise the boundary".into(),
                    ),
                });
            }
        }
    }

    report.files_checked = edges
        .iter()
        .map(|(f, _, _)| f.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();

    report.sources_run.push("boundary-violations".into());
    report
}
