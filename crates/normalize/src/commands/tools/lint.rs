//! Lint command - run linters, formatters, and type checkers.

use crate::output::OutputFormatter;
use normalize_tools::{ToolCategory, registry_with_custom};
use rayon::prelude::*;
use serde::Serialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Error type for lint run operations.
#[derive(Debug, thiserror::Error)]
pub enum LintError {
    /// No tools were detected for the given root directory.
    #[error("no lint tools detected in {root}")]
    NoToolsDetected {
        /// The root directory that was scanned.
        root: String,
    },
    /// A tool filter name did not match any registered tool.
    #[error("unknown tool '{name}'")]
    UnknownTool {
        /// The tool name that was not found.
        name: String,
    },
    /// An unknown category filter was supplied.
    #[error("unknown category '{category}'; expected one of: lint, fmt, type")]
    UnknownCategory {
        /// The category string that was not recognised.
        category: String,
    },
}

/// Tool info for lint list output
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ToolListItem {
    /// Tool name (e.g. "clippy", "eslint").
    pub name: String,
    /// Tool category: one of "lint", "fmt", or "type".
    pub category: String,
    /// Whether the tool binary was found on `PATH` and is executable.
    pub available: bool,
    /// Version string reported by the tool, or `None` if not installed.
    pub version: Option<String>,
    /// Comma-separated list of file extensions this tool handles (e.g. "rs" or "js, ts").
    pub extensions: String,
    /// URL of the tool's documentation or home page.
    pub website: String,
}

/// Report for lint list command
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintListReport {
    pub tools: Vec<ToolListItem>,
}

impl OutputFormatter for LintListReport {
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
            let _ = writeln!(out, "    Extensions: {}", tool.extensions);
            let _ = writeln!(out, "    Website: {}", tool.website);
            let _ = writeln!(out);
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
pub fn build_lint_list(root: Option<&Path>) -> LintListReport {
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

    LintListReport { tools }
}

/// Per-repo lint entry for multi-repo mode.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RepoLintEntry {
    /// Repository directory name (last component of `path`).
    pub name: String,
    /// Absolute path to the repository root.
    pub path: PathBuf,
    /// Set when the entire repo could not be linted (e.g. no tools detected).
    /// When `Some`, `diagnostics` will be empty and counts will be zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of error-severity diagnostics produced for this repo.
    pub error_count: usize,
    /// Number of warning-severity diagnostics produced for this repo.
    pub warning_count: usize,
    /// Individual diagnostics from all tools run against this repo.
    pub diagnostics: Vec<LintDiagnostic>,
}

/// Report for lint run command with structured diagnostics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintRunReport {
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<LintDiagnostic>,
    /// Populated in multi-repo mode (--repos-dir).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<RepoLintEntry>>,
    /// True when --fix was requested but --dry-run prevented writes.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// A lint diagnostic for service output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LintDiagnostic {
    /// Name of the tool that produced this diagnostic (e.g. "clippy", "eslint").
    pub tool: String,
    /// Free-form severity string as reported by the tool (e.g. "error", "warning", "note").
    pub severity: String,
    /// Rule or check identifier. May be empty if the tool does not emit rule IDs.
    pub rule_id: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Path to the file containing the issue, relative to the repository root.
    pub file: String,
    /// 1-based line number of the diagnostic location.
    pub line: usize,
    /// 1-based column number of the diagnostic location.
    pub column: usize,
    /// Optional URL linking to documentation for the rule or error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
}

impl OutputFormatter for LintRunReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.dry_run {
            let _ = writeln!(out, "(dry run — no files were modified)\n");
        }
        if let Some(repos) = &self.repos {
            // Multi-repo mode
            if repos.is_empty() {
                return "No repositories found".to_string();
            }
            for repo in repos {
                let _ = writeln!(out, "=== {} ===", repo.name);
                if let Some(err) = &repo.error {
                    let _ = writeln!(out, "Error: {}", err);
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
                        let _ = writeln!(out);
                        writeln!(
                            out,
                            "Found {} error(s) and {} warning(s)",
                            repo.error_count, repo.warning_count
                        )
                        .unwrap();
                    }
                }
                let _ = writeln!(out);
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
                let _ = writeln!(out);
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

/// Run lints across multiple repos and return aggregated results.
pub fn build_lint_run_multi(
    repos: &[PathBuf],
    fix: bool,
    dry_run: bool,
    tools: Option<&str>,
    category: Option<&str>,
) -> Result<LintRunReport, LintError> {
    let entries: Vec<RepoLintEntry> = repos
        .par_iter()
        .map(|repo_path| {
            let name = repo_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            match build_lint_run(None, Some(repo_path), fix, dry_run, tools, category) {
                Ok(r) => RepoLintEntry {
                    name,
                    path: repo_path.clone(),
                    error: None,
                    error_count: r.error_count,
                    warning_count: r.warning_count,
                    diagnostics: r.diagnostics,
                },
                Err(e) => RepoLintEntry {
                    name,
                    path: repo_path.clone(),
                    error: Some(e.to_string()),
                    error_count: 0,
                    warning_count: 0,
                    diagnostics: vec![],
                },
            }
        })
        .collect();

    let total_errors: usize = entries.iter().map(|r| r.error_count).sum();
    let total_warnings: usize = entries.iter().map(|r| r.warning_count).sum();

    Ok(LintRunReport {
        error_count: total_errors,
        warning_count: total_warnings,
        diagnostics: vec![],
        repos: Some(entries),
        dry_run,
    })
}

/// Run lints and return structured results (data only).
pub fn build_lint_run(
    target: Option<&str>,
    root: Option<&Path>,
    fix: bool,
    dry_run: bool,
    tools: Option<&str>,
    category: Option<&str>,
) -> Result<LintRunReport, LintError> {
    let root = root.unwrap_or_else(|| Path::new("."));
    let registry = registry_with_custom(root);

    let category_filter: Option<ToolCategory> = if let Some(c) = category {
        match c {
            "lint" | "linter" => Some(ToolCategory::Linter),
            "fmt" | "format" | "formatter" => Some(ToolCategory::Formatter),
            "type" | "typecheck" | "type-checker" => Some(ToolCategory::TypeChecker),
            _ => {
                return Err(LintError::UnknownCategory {
                    category: c.to_string(),
                });
            }
        }
    } else {
        None
    };

    let tools_to_run: Vec<&dyn normalize_tools::Tool> = if let Some(tool_names) = tools {
        let names: Vec<&str> = tool_names.split(',').map(|s| s.trim()).collect();
        if let Some(unknown) = names
            .iter()
            .find(|&&n| !registry.tools().iter().any(|t| t.info().name == n))
        {
            return Err(LintError::UnknownTool {
                name: unknown.to_string(),
            });
        }
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
        return Ok(LintRunReport {
            error_count: 0,
            warning_count: 0,
            diagnostics: vec![],
            repos: None,
            dry_run,
        });
    }

    // When dry_run is set, run in check mode only (no writes) regardless of fix flag.
    let paths: Vec<&Path> = target.map(|t| vec![Path::new(t)]).unwrap_or_default();
    let effective_fix = fix && !dry_run;
    let all_results = run_tools(&tools_to_run, &paths, effective_fix, false, root).all_results;

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

    Ok(LintRunReport {
        error_count,
        warning_count,
        diagnostics,
        repos: None,
        dry_run,
    })
}
