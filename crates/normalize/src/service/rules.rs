//! Rules management service for server-less CLI.

use super::resolve_pretty;
use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// Rules management sub-service.
pub struct RulesService {
    pretty: Cell<bool>,
    sarif: Cell<bool>,
    /// Maximum issues to display (None = all). 0 also means all (per --limit 0 convention).
    limit: Cell<Option<usize>>,
}

impl RulesService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            sarif: Cell::new(false),
            limit: Cell::new(None),
        }
    }

    fn display_rules_run(&self, r: &DiagnosticsReport) -> String {
        if self.sarif.get() {
            r.format_sarif()
        } else if self.pretty.get() {
            r.format_pretty()
        } else {
            // 0 means show all; otherwise cap at the given limit
            let lim = self
                .limit
                .get()
                .and_then(|n| if n == 0 { None } else { Some(n) });
            r.format_text_limited(lim)
        }
    }
}

/// Generic result type for rule operations.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RuleResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for RuleResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else if self.success {
            write!(f, "Done")
        } else {
            write!(f, "Failed")
        }
    }
}

#[cli(
    name = "rules",
    about = "Manage and run analysis rules (syntax + fact)"
)]
impl RulesService {
    /// List all rules (syntax + fact, builtin + user)
    #[allow(clippy::too_many_arguments)]
    pub fn list(
        &self,
        #[param(help = "Show source URLs for imported rules")] sources: bool,
        #[param(short = 'e', help = "Filter by engine (all, syntax, fact, sarif)")] engine: Option<
            crate::commands::rules::RuleType,
        >,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Filter to enabled rules only")] enabled: bool,
        #[param(help = "Filter to disabled rules only")] disabled: bool,
        #[param(help = "Hide the description line")] no_desc: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_list_service(
            root.as_deref(),
            sources,
            engine.unwrap_or_default(),
            tag.as_deref(),
            enabled,
            disabled,
            no_desc,
            resolve_pretty(
                root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
                pretty,
                compact,
            ),
        )
    }

    /// Run rules against the codebase
    #[allow(clippy::too_many_arguments)]
    #[cli(display_with = "display_rules_run")]
    pub fn run(
        &self,
        #[param(help = "Specific rule ID to run")] rule: Option<String>,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Apply auto-fixes (syntax rules only)")] fix: bool,
        #[param(help = "Output in SARIF format")] sarif: bool,
        #[param(positional, help = "Target directory or file")] target: Option<String>,
        #[param(short = 'e', help = "Filter by engine (all, syntax, fact, sarif)")] engine: Option<
            crate::commands::rules::RuleType,
        >,
        #[param(help = "Maximum issues to display (0 = show all, default: 50)")] limit: Option<
            usize,
        >,
        #[param(help = "Exit 0 even when error-severity issues are found")] no_fail: bool,
        #[param(help = "Debug flags (comma-separated)")] debug: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<DiagnosticsReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd was deleted (OS-level failure)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let target_root = target
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| effective_root.clone());
        let config = crate::config::NormalizeConfig::load(&effective_root);

        self.pretty
            .set(resolve_pretty(&target_root, pretty, compact));
        self.sarif.set(sarif);
        // Default limit of 50 when not specified; 0 means show all
        self.limit.set(Some(limit.unwrap_or(50)));

        // --fix is a separate mutation path: apply fixes and return a simple message.
        // Loop until no fixable issues remain (each pass resolves the innermost
        // layer of nested violations; outer violations are deferred and picked
        // up by subsequent passes with fresh byte offsets).
        if fix {
            let debug_flags = normalize_syntax_rules::DebugFlags::from_args(&debug);
            let run = || {
                crate::commands::analyze::rules_cmd::run_syntax_rules(
                    &target_root,
                    rule.as_deref(),
                    tag.as_deref(),
                    None,
                    &config.analyze.rules,
                    &debug_flags,
                )
            };
            let mut total_fixed = 0;
            let mut total_files = 0;
            loop {
                let findings = run();
                let fixable_count = findings.iter().filter(|f| f.fix.is_some()).count();
                if fixable_count == 0 {
                    break;
                }
                match normalize_syntax_rules::apply_fixes(&findings) {
                    Ok(0) => break,
                    Ok(files_modified) => {
                        total_fixed += fixable_count;
                        total_files = files_modified;
                    }
                    Err(e) => return Err(format!("Error applying fixes: {e}")),
                }
            }
            if total_fixed == 0 {
                return Ok(DiagnosticsReport {
                    issues: Vec::new(),
                    files_checked: 0,
                    sources_run: vec!["syntax-rules (fix: no fixable issues)".into()],
                    hints: Vec::new(),
                });
            }
            return Ok(DiagnosticsReport {
                issues: Vec::new(),
                files_checked: total_files,
                sources_run: vec![format!(
                    "syntax-rules (fixed {} issue(s) in {} file(s))",
                    total_fixed, total_files
                )],
                hints: Vec::new(),
            });
        }

        let engine_filter = engine.unwrap_or_default();
        let mut report = crate::commands::rules::run_rules_report(
            &target_root,
            rule.as_deref(),
            tag.as_deref(),
            &engine_filter,
            &debug,
            &config,
        );

        let error_count = report.count_by_severity(normalize_output::diagnostics::Severity::Error);
        if !no_fail && error_count > 0 {
            return Err(format!("{error_count} error(s) found"));
        }

        if !report.issues.is_empty() && !self.pretty.get() && !self.sarif.get() {
            report
                .hints
                .push("Run `normalize rules run --pretty` for a detailed view".to_string());
            let has_syntax_engine = matches!(
                engine_filter,
                crate::commands::rules::RuleType::All | crate::commands::rules::RuleType::Syntax
            );
            if has_syntax_engine {
                report.hints.push(
                    "Run `normalize rules run --fix` to auto-fix style violations".to_string(),
                );
            }
        }

        Ok(report)
    }

    /// Enable a rule or all rules matching a tag
    pub fn enable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_enable_disable_service(
            root.as_deref(),
            &id_or_tag,
            true,
            dry_run,
        )
    }

    /// Disable a rule or all rules matching a tag
    pub fn disable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_enable_disable_service(
            root.as_deref(),
            &id_or_tag,
            false,
            dry_run,
        )
    }

    /// Show full documentation for a rule
    pub fn show(
        &self,
        #[param(positional, help = "Rule ID to show")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_show_service(
            root.as_deref(),
            &id,
            resolve_pretty(
                root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
                pretty,
                compact,
            ),
        )
    }

    /// List all tags and the rules they group
    pub fn tags(
        &self,
        #[param(help = "Expand each tag to show its member rules")] show_rules: bool,
        #[param(help = "Show only this specific tag")] tag: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_tags_service(
            root.as_deref(),
            show_rules,
            tag.as_deref(),
            resolve_pretty(
                root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
                pretty,
                compact,
            ),
        )
    }

    /// Add a rule from a URL
    pub fn add(
        &self,
        #[param(positional, help = "URL to download the rule from")] url: String,
        #[param(help = "Install to global rules instead of project")] global: bool,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_add_service(&url, global)
    }

    /// Update imported rules from their sources
    pub fn update(
        &self,
        #[param(
            positional,
            help = "Specific rule ID to update (updates all if omitted)"
        )]
        rule_id: Option<String>,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_update_service(rule_id.as_deref())
    }

    /// Remove an imported rule
    pub fn remove(
        &self,
        #[param(positional, help = "Rule ID to remove")] rule_id: String,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_remove_service(&rule_id)
    }
}
