//! `dead-parameter` native rule — flags function parameters never referenced in the function body.
//!
//! Uses tree-sitter `locals.scm` queries (via `normalize-scope`) to find parameters
//! defined with `@local.definition.parameter` that have no resolved reference in the
//! same file. Underscore-prefixed names (`_`, `_unused`) are excluded — those are the
//! conventional way to mark intentionally unused parameters.
//!
//! # Language support
//!
//! Requires `@local.definition.parameter` captures in the language's `locals.scm`.
//! Currently supported: Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C, C++, C#.
//! Languages without this capture are silently skipped.

use normalize_languages::parsers::grammar_loader;
use normalize_languages::support_for_path;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use normalize_scope::ScopeEngine;
use std::path::Path;

use crate::cache::{FileRule, run_file_rule};
use normalize_rules_config::WalkConfig;

/// Serializable per-file finding for the dead-parameter rule.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DeadParameterFinding {
    rel_path: String,
    /// Name of the unused parameter.
    name: String,
    /// 1-based line number where the parameter is defined.
    line: usize,
}

/// Rule that flags function parameters never referenced in their function body.
pub struct DeadParameterRule;

impl FileRule for DeadParameterRule {
    type Finding = DeadParameterFinding;

    fn engine_name(&self) -> &str {
        "dead-parameter"
    }

    fn config_hash(&self) -> String {
        // No configurable threshold; any change to the rule source invalidates the cache.
        "1".into()
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

        let loader = grammar_loader();
        let engine = ScopeEngine::new(&loader);
        let lang = support.grammar_name();

        // Skip languages that have no locals.scm (engine returns empty for those).
        if !engine.has_locals(lang) {
            return Vec::new();
        }

        let unused = engine.find_unused_parameters(lang, &content);
        if unused.is_empty() {
            return Vec::new();
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        unused
            .into_iter()
            .map(|def| DeadParameterFinding {
                rel_path: rel_path.clone(),
                name: def.name,
                line: def.location.line,
            })
            .collect()
    }

    fn to_diagnostics(
        &self,
        findings: Vec<(std::path::PathBuf, Vec<Self::Finding>)>,
        _root: &Path,
        files_checked: usize,
    ) -> DiagnosticsReport {
        let issues: Vec<Issue> = findings
            .into_iter()
            .flat_map(|(_path, file_findings)| file_findings)
            .map(|f| Issue {
                file: f.rel_path,
                line: Some(f.line),
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "dead-parameter".into(),
                message: format!("parameter `{}` is never used", f.name),
                severity: Severity::Warning,
                source: "dead-parameter".into(),
                related: vec![],
                suggestion: Some(
                    "prefix with `_` to mark it intentionally unused, or remove it if possible"
                        .into(),
                ),
            })
            .collect();

        DiagnosticsReport {
            issues,
            files_checked,
            sources_run: vec!["dead-parameter".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}

/// Build a `DiagnosticsReport` for the `dead-parameter` rule.
///
/// Walks all source files under `root`, analyzes each with the scope engine, and emits
/// a warning for every function parameter that is never referenced in the file.
pub fn build_dead_parameter_report(
    root: &Path,
    explicit_files: Option<&[std::path::PathBuf]>,
    walk_config: &WalkConfig,
) -> DiagnosticsReport {
    let rule = DeadParameterRule;
    run_file_rule(&rule, root, explicit_files, walk_config)
}
