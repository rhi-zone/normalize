//! `long-file` native rule — flags source files exceeding a line count threshold.
//!
//! Respects `.normalize/large-files-allow` allowlist and excludes lock files.
//!
//! # Configuration
//!
//! The threshold is configurable via `.normalize/config.toml`:
//!
//! ```toml
//! [rules.rule."long-file"]
//! threshold = 300   # default: 500
//! ```

use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use std::path::Path;

use crate::cache::{FileRule, run_file_rule};
use normalize_rules_config::WalkConfig;

/// Serializable per-file finding for the long-file rule.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct LongFileFinding {
    rel_path: String,
    line_count: usize,
}

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

/// Rule that flags source files exceeding a line count threshold.
pub struct LongFileRule {
    pub threshold: usize,
    allow_patterns: Vec<glob::Pattern>,
}

impl LongFileRule {
    /// Create a new `LongFileRule`, loading allowlist patterns from the project root.
    pub fn new(threshold: usize, root: &Path) -> Self {
        Self {
            threshold,
            allow_patterns: load_allow_patterns(root),
        }
    }
}

impl FileRule for LongFileRule {
    type Finding = LongFileFinding;

    fn engine_name(&self) -> &str {
        "long-file"
    }

    fn config_hash(&self) -> String {
        self.threshold.to_string()
    }

    fn check_file(&self, path: &Path, root: &Path) -> Vec<Self::Finding> {
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
            return Vec::new();
        }

        // Skip allowlisted paths.
        if self.allow_patterns.iter().any(|p| p.matches(&rel_path)) {
            return Vec::new();
        }

        let lines = match std::fs::read_to_string(path) {
            Ok(content) => content.lines().count(),
            Err(_) => return Vec::new(),
        };

        if lines >= self.threshold {
            vec![LongFileFinding {
                rel_path,
                line_count: lines,
            }]
        } else {
            Vec::new()
        }
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
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "long-file".into(),
                message: format!("{} lines (threshold: {threshold})", f.line_count),
                severity: Severity::Warning,
                source: "long-file".into(),
                related: vec![],
                suggestion: Some("consider splitting into smaller, focused modules".into()),
            })
            .collect();

        // Sort by line count descending.
        issues.sort_by(|a, b| {
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
            sources_run: vec!["long-file".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}

/// Build a `DiagnosticsReport` for the `long-file` rule.
///
/// Walks all source files under `root`, counts lines, and emits an issue for
/// each file exceeding the threshold. Lock files and allowlisted paths are
/// skipped.
pub fn build_long_file_report(
    root: &Path,
    threshold: usize,
    files: Option<&[std::path::PathBuf]>,
    walk_config: &WalkConfig,
) -> DiagnosticsReport {
    let rule = LongFileRule::new(threshold, root);
    run_file_rule(&rule, root, files, walk_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn make_file_with_lines(dir: &std::path::Path, name: &str, n: usize) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..n {
            writeln!(f, "line {i}").unwrap();
        }
        path
    }

    #[test]
    fn test_default_threshold_not_triggered() {
        let dir = tempfile::tempdir().unwrap();
        // 499 lines — below default threshold of 500
        let path = make_file_with_lines(dir.path(), "short.rs", 499);
        let rule = LongFileRule::new(500, dir.path());
        let findings = rule.check_file(&path, dir.path());
        assert!(
            findings.is_empty(),
            "499 lines should not trigger default threshold of 500"
        );
    }

    #[test]
    fn test_default_threshold_triggered() {
        let dir = tempfile::tempdir().unwrap();
        // 500 lines — at default threshold
        let path = make_file_with_lines(dir.path(), "long.rs", 500);
        let rule = LongFileRule::new(500, dir.path());
        let findings = rule.check_file(&path, dir.path());
        assert_eq!(
            findings.len(),
            1,
            "500 lines should trigger default threshold of 500"
        );
    }

    #[test]
    fn test_custom_threshold_lower() {
        let dir = tempfile::tempdir().unwrap();
        // 100 lines — below default but above custom threshold of 50
        let path = make_file_with_lines(dir.path(), "medium.rs", 100);
        let rule = LongFileRule::new(50, dir.path());
        let findings = rule.check_file(&path, dir.path());
        assert_eq!(
            findings.len(),
            1,
            "100 lines should trigger custom threshold of 50"
        );
    }

    #[test]
    fn test_custom_threshold_higher() {
        let dir = tempfile::tempdir().unwrap();
        // 500 lines — at default but below custom threshold of 1000
        let path = make_file_with_lines(dir.path(), "medium.rs", 500);
        let rule = LongFileRule::new(1000, dir.path());
        let findings = rule.check_file(&path, dir.path());
        assert!(
            findings.is_empty(),
            "500 lines should not trigger custom threshold of 1000"
        );
    }
}
