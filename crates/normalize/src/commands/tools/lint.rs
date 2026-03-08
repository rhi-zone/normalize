//! Lint command - run linters, formatters, and type checkers.

use crate::output::OutputFormatter;
use normalize_tools::{ToolCategory, registry_with_custom};
use rayon::prelude::*;
use serde::Serialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Tool info for lint list output
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ToolListItem {
    pub name: String,
    pub category: String,
    pub available: bool,
    pub version: Option<String>,
    pub extensions: String,
    pub website: String,
}

/// Result of lint list command
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintListResult {
    pub tools: Vec<ToolListItem>,
}

impl OutputFormatter for LintListResult {
    fn format_text(&self) -> String {
        let mut out = String::from("Detected tools:\n\n");
        for tool in &self.tools {
            let status = if tool.available { "✓" } else { "✗" };
            let ver = tool.version.as_deref().unwrap_or("not installed");
            writeln!(
                out,
                "  {} {} ({}) - {}",
                status, tool.name, tool.category, ver
            )
            .unwrap();
            writeln!(out, "    Extensions: {}", tool.extensions).unwrap();
            writeln!(out, "    Website: {}", tool.website).unwrap();
            writeln!(out).unwrap();
        }
        out
    }
}

struct ToolRunResult {
    all_results: Vec<normalize_tools::ToolResult>,
}

/// Run a set of tools against `paths` and collect results.
fn run_tools(
    tools_to_run: &[&dyn normalize_tools::Tool],
    paths: &[&Path],
    fix: bool,
    json: bool,
    root: &Path,
) -> ToolRunResult {
    let mut all_results = Vec::new();

    for tool in tools_to_run {
        let info = tool.info();

        if !tool.is_available() {
            if !json {
                eprintln!("{}: not installed", info.name);
            }
            continue;
        }

        if !json {
            let action = if fix && tool.can_fix() {
                "fixing"
            } else {
                "checking"
            };
            eprintln!("{}: {}...", info.name, action);
        }

        let result = if fix && tool.can_fix() {
            tool.fix(paths, root)
        } else {
            tool.run(paths, root)
        };

        match result {
            Ok(result) => {
                if !result.success
                    && let Some(err) = &result.error
                    && !json
                {
                    eprintln!("{}: {}", info.name, err);
                }
                all_results.push(result);
            }
            Err(e) => {
                if !json {
                    eprintln!("{}: {}", info.name, e);
                }
            }
        }
    }

    ToolRunResult { all_results }
}

/// Build lint list report (data only, no printing).
pub fn build_lint_list(root: Option<&Path>) -> LintListResult {
    let root = root.unwrap_or_else(|| Path::new("."));
    let registry = registry_with_custom(root);

    let detected = registry.detect(root);
    let tools: Vec<ToolListItem> = detected
        .par_iter()
        .map(|(t, _)| {
            let info = t.info();
            let version = t.version();
            ToolListItem {
                name: info.name.to_string(),
                category: info.category.as_str().to_string(),
                available: version.is_some(),
                version,
                extensions: info.extensions.join(", "),
                website: info.website.to_string(),
            }
        })
        .collect();

    LintListResult { tools }
}

/// Per-repo lint result for multi-repo mode.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoLintResult {
    pub name: String,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<LintDiagnostic>,
}

/// Lint run result with structured diagnostics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintRunResult {
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<LintDiagnostic>,
    /// Populated in multi-repo mode (--repos-dir).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<RepoLintResult>>,
}

/// A lint diagnostic for service output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintDiagnostic {
    pub tool: String,
    pub severity: String,
    pub rule_id: String,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
}

impl OutputFormatter for LintRunResult {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if let Some(repos) = &self.repos {
            // Multi-repo mode
            if repos.is_empty() {
                return "No repositories found".to_string();
            }
            for repo in repos {
                writeln!(out, "=== {} ===", repo.name).unwrap();
                if let Some(err) = &repo.error {
                    writeln!(out, "Error: {}", err).unwrap();
                } else {
                    for diag in &repo.diagnostics {
                        writeln!(
                            out,
                            "{}:{}:{}: {} [{}] {}",
                            diag.file,
                            diag.line,
                            diag.column,
                            diag.severity,
                            diag.rule_id,
                            diag.message
                        )
                        .unwrap();
                    }
                    if repo.error_count > 0 || repo.warning_count > 0 {
                        writeln!(out).unwrap();
                        writeln!(
                            out,
                            "Found {} error(s) and {} warning(s)",
                            repo.error_count, repo.warning_count
                        )
                        .unwrap();
                    }
                }
                writeln!(out).unwrap();
            }
        } else {
            // Single-repo mode
            for diag in &self.diagnostics {
                writeln!(
                    out,
                    "{}:{}:{}: {} [{}] {}",
                    diag.file, diag.line, diag.column, diag.severity, diag.rule_id, diag.message
                )
                .unwrap();
            }
            if self.error_count > 0 || self.warning_count > 0 {
                writeln!(out).unwrap();
                write!(
                    out,
                    "Found {} error(s) and {} warning(s)",
                    self.error_count, self.warning_count
                )
                .unwrap();
            }
        }
        out
    }
}

impl std::fmt::Display for LintRunResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Run lints across multiple repos and return aggregated results.
pub fn build_lint_run_multi(
    repos: &[PathBuf],
    fix: bool,
    tools: Option<&str>,
    category: Option<&str>,
) -> Result<LintRunResult, String> {
    let entries: Vec<RepoLintResult> = repos
        .par_iter()
        .map(|repo_path| {
            let name = repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            match build_lint_run(None, Some(repo_path), fix, tools, category) {
                Ok(r) => RepoLintResult {
                    name,
                    path: repo_path.clone(),
                    error: None,
                    error_count: r.error_count,
                    warning_count: r.warning_count,
                    diagnostics: r.diagnostics,
                },
                Err(e) => RepoLintResult {
                    name,
                    path: repo_path.clone(),
                    error: Some(e),
                    error_count: 0,
                    warning_count: 0,
                    diagnostics: vec![],
                },
            }
        })
        .collect();

    let total_errors: usize = entries.iter().map(|r| r.error_count).sum();
    let total_warnings: usize = entries.iter().map(|r| r.warning_count).sum();

    Ok(LintRunResult {
        error_count: total_errors,
        warning_count: total_warnings,
        diagnostics: vec![],
        repos: Some(entries),
    })
}

/// Run lints and return structured results (data only).
pub fn build_lint_run(
    target: Option<&str>,
    root: Option<&Path>,
    fix: bool,
    tools: Option<&str>,
    category: Option<&str>,
) -> Result<LintRunResult, String> {
    let root = root.unwrap_or_else(|| Path::new("."));
    let registry = registry_with_custom(root);

    let category_filter: Option<ToolCategory> = category.and_then(|c| match c {
        "lint" | "linter" => Some(ToolCategory::Linter),
        "fmt" | "format" | "formatter" => Some(ToolCategory::Formatter),
        "type" | "typecheck" | "type-checker" => Some(ToolCategory::TypeChecker),
        _ => None,
    });

    let tools_to_run: Vec<&dyn normalize_tools::Tool> = if let Some(tool_names) = tools {
        let names: Vec<&str> = tool_names.split(',').map(|s| s.trim()).collect();
        registry
            .tools()
            .iter()
            .filter(|t| names.contains(&t.info().name))
            .map(|t| t.as_ref())
            .collect()
    } else {
        let detected = registry.detect(root);
        detected
            .into_iter()
            .filter(|(t, _)| {
                if let Some(cat) = category_filter {
                    t.info().category == cat
                } else {
                    true
                }
            })
            .map(|(t, _)| t)
            .collect()
    };

    if tools_to_run.is_empty() {
        return Ok(LintRunResult {
            error_count: 0,
            warning_count: 0,
            diagnostics: vec![],
            repos: None,
        });
    }

    let paths: Vec<&Path> = target.map(|t| vec![Path::new(t)]).unwrap_or_default();
    let all_results = run_tools(&tools_to_run, &paths, fix, false, root).all_results;

    let mut diagnostics = Vec::new();
    let mut error_count = 0;
    let mut warning_count = 0;

    for result in &all_results {
        error_count += result.error_count();
        warning_count += result.warning_count();
        for diag in &result.diagnostics {
            diagnostics.push(LintDiagnostic {
                tool: diag.tool.clone(),
                severity: diag.severity.as_str().to_string(),
                rule_id: diag.rule_id.clone(),
                message: diag.message.clone(),
                file: diag.location.file.display().to_string(),
                line: diag.location.line,
                column: diag.location.column,
                help_url: diag.help_url.clone(),
            });
        }
    }

    Ok(LintRunResult {
        error_count,
        warning_count,
        diagnostics,
        repos: None,
    })
}
