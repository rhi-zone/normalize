//! `large-file` native rule — flags source files exceeding a line count threshold.
//!
//! Respects `.normalize/large-files-allow` allowlist and excludes lock files.

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use std::path::Path;

use crate::walk::gitignore_walk;

/// Well-known lock files that should never be flagged as large.
fn is_lockfile(name: &str) -> bool {
    matches!(
        name,
        "uv.lock"
            | "Cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lockb"
            | "bun.lock"
            | "poetry.lock"
            | "Pipfile.lock"
            | "Gemfile.lock"
            | "composer.lock"
            | "go.sum"
            | "flake.lock"
            | "packages.lock.json"
            | "paket.lock"
            | "pubspec.lock"
            | "mix.lock"
            | "rebar.lock"
            | "Podfile.lock"
            | "shrinkwrap.yaml"
            | "deno.lock"
            | "gradle.lockfile"
    )
}

/// Load glob patterns from `.normalize/large-files-allow`.
fn load_allow_patterns(root: &Path) -> Vec<glob::Pattern> {
    let path = root.join(".normalize").join("large-files-allow");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .filter_map(|line| glob::Pattern::new(line.trim()).ok())
        .collect()
}

/// Build a `DiagnosticsReport` for the `large-file` rule.
///
/// Walks all source files under `root`, counts lines, and emits an issue for
/// each file exceeding the threshold. Lock files and allowlisted paths are
/// skipped.
pub fn build_large_file_report(root: &Path, threshold: usize) -> DiagnosticsReport {
    let allow_patterns = load_allow_patterns(root);

    let mut issues = Vec::new();
    let mut files_checked = 0usize;

    for entry in gitignore_walk(root) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Only check files we recognize as source code.
        if normalize_languages::support_for_path(path).is_none() {
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Skip lock files.
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if is_lockfile(&file_name) {
            continue;
        }

        // Skip allowlisted paths.
        if allow_patterns.iter().any(|p| p.matches(&rel_path)) {
            continue;
        }

        files_checked += 1;

        let lines = match std::fs::read_to_string(path) {
            Ok(content) => content.lines().count(),
            Err(_) => continue,
        };

        if lines >= threshold {
            issues.push(Issue {
                file: rel_path,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "large-file".into(),
                message: format!("{lines} lines (threshold: {threshold})"),
                severity: Severity::Warning,
                source: "large-file".into(),
                related: vec![],
                suggestion: Some("consider splitting into smaller, focused modules".into()),
            });
        }
    }

    // Sort by line count descending.
    issues.sort_by(|a, b| {
        // Extract line count from message for sorting.
        let a_lines: usize = a
            .message
            .split(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let b_lines: usize = b
            .message
            .split(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        b_lines.cmp(&a_lines)
    });

    DiagnosticsReport {
        issues,
        files_checked,
        sources_run: vec!["large-file".into()],
        tool_errors: vec![],
    }
}
